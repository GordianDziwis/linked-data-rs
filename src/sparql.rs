mod rdf_type_conversions;
use rdf_types::generator::Blank;
use rdf_types::interpretation::WithGenerator;
use rdf_types::RdfDisplay;
use spargebra::algebra::{Expression, GraphPattern};
use spargebra::term::{NamedNode, NamedNodePattern, TermPattern, TriplePattern, Variable};
use spargebra::Query;
use sparopt::Optimizer;
use uuid::Uuid;

use crate::{to_quads_with, LinkedData};

#[derive(Default)]
struct ConstructQuery {
	construct_template: Vec<TriplePattern>,
	where_pattern: GraphPattern,
}

pub trait SparqlQuery {
	fn sparql_query() -> String {
		Self::sparql_algebra().to_string()
	}

	fn as_sparql_query(&self) -> String {
		self.as_sparql_algebra().to_string()
	}

	fn sparql_algebra() -> Query;

	fn as_sparql_algebra(&self) -> Query {
		Self::sparql_algebra()
	}
}

trait ToConstructQuery {
	fn to_query_with_binding(binding_variable: Variable) -> ConstructQuery;

	fn to_query() -> ConstructQuery {
		Self::to_query_with_binding(generate_unique_variable())
	}
}

trait And {
	fn and(self, other: Self) -> Self;
}

trait Join {
	fn join(self, other: Self) -> Self;
}

trait Union {
	fn union(self, other: Self) -> Self;
}

impl ConstructQuery {
	fn new(
		subject: impl Into<TermPattern>,
		predicate: impl Into<NamedNodePattern>,
		object: impl Into<TermPattern>,
	) -> ConstructQuery {
		let patterns = vec![TriplePattern {
			subject: subject.into(),
			predicate: predicate.into(),
			object: object.into(),
		}];
		ConstructQuery {
			construct_template: patterns.clone(),
			where_pattern: GraphPattern::Bgp { patterns },
		}
	}

	fn new_with_binding<F>(
		subject: Variable,
		predicate: NamedNode,
		to_query_with_binding: F,
	) -> Self
	where
		F: FnOnce(Variable) -> Self,
	{
		let object = generate_unique_variable();
		ConstructQuery::new(subject, predicate, object.clone()).join(to_query_with_binding(object))
	}

	fn union_with_binding<F>(
		self,
		subject: Variable,
		predicate: NamedNode,
		to_query_with_binding: F,
	) -> Self
	where
		F: FnOnce(Variable) -> Self,
	{
		let object = generate_unique_variable();
		self.union(ConstructQuery::new(subject, predicate, object.clone()))
			.join(to_query_with_binding(object))
	}

	fn join_with_binding<F>(
		self,
		subject: Variable,
		predicate: NamedNode,
		to_query_with_binding: F,
	) -> Self
	where
		F: FnOnce(Variable) -> Self,
	{
		let object = generate_unique_variable();
		self.join(ConstructQuery::new(subject, predicate, object.clone()))
			.join(to_query_with_binding(object))
	}

	fn join_with(self, subject: Variable, predicate: NamedNode, object: NamedNode) -> Self {
		self.join(ConstructQuery::new(subject, predicate, object))
	}

	fn filter_variable(self, variable: Variable, id: NamedNode) -> Self {
		let expr = Expression::Equal(
			Box::new(Expression::Variable(variable)),
			Box::new(Expression::NamedNode(id)),
		);
		Self {
			construct_template: self.construct_template,
			where_pattern: GraphPattern::Filter {
				expr,
				inner: Box::new(self.where_pattern),
			},
		}
	}
}

impl From<ConstructQuery> for Query {
	fn from(value: ConstructQuery) -> Self {
		// TODO Remove the optimizer
		let pattern = (&Optimizer::optimize_graph_pattern((&value.where_pattern).into())).into();
		Query::Construct {
			template: value.construct_template,
			dataset: None,
			pattern,
			base_iri: None,
		}
	}
}

impl Join for ConstructQuery {
	fn join(mut self, other: Self) -> Self {
		self.construct_template = self.construct_template.and(other.construct_template);
		self.where_pattern = self.where_pattern.join(other.where_pattern);
		self
	}
}

impl Union for ConstructQuery {
	fn union(mut self, other: Self) -> Self {
		self.construct_template = self.construct_template.and(other.construct_template);
		self.where_pattern = self.where_pattern.union(other.where_pattern);
		self
	}
}

impl<T> SparqlQuery for T
where
	T: ToConstructQuery,
{
	fn sparql_algebra() -> Query {
		Self::to_query().into()
	}
}

impl And for Vec<TriplePattern> {
	fn and(mut self, other: Self) -> Self {
		self.extend(other);
		self
	}
}

impl Join for GraphPattern {
	fn join(self, other: Self) -> Self {
		GraphPattern::Join {
			left: Box::new(self),
			right: Box::new(other),
		}
	}
}

impl Union for GraphPattern {
	fn union(self, other: Self) -> Self {
		GraphPattern::Union {
			left: Box::new(self),
			right: Box::new(other),
		}
	}
}

impl ToConstructQuery for Variable {
	fn to_query_with_binding(_: Variable) -> ConstructQuery {
		ConstructQuery::default()
	}
}

impl ToConstructQuery for String {
	fn to_query_with_binding(_: Variable) -> ConstructQuery {
		ConstructQuery::default()
	}
}

fn with_predicate<F>(
	predicate: NamedNode,
	to_query_with_binding: F,
) -> impl FnOnce(Variable) -> ConstructQuery
where
	F: FnOnce(Variable) -> ConstructQuery,
{
	|subject| {
		let object = generate_unique_variable();
		ConstructQuery::new(subject, predicate, object.clone()).join(to_query_with_binding(object))
	}
}

fn generate_unique_variable() -> Variable {
	let uuid = format!("{}", Uuid::new_v4().simple());
	// TODO Avoid collisons
	let variable = uuid[..5].to_string();
	Variable::new(variable).expect("XXX")
}

pub fn to_nquads(value: &impl LinkedData<WithGenerator<Blank>>) -> String {
	let mut interpretation = WithGenerator::new((), Blank::new());
	to_quads_with(&mut (), &mut interpretation, value)
		.unwrap()
		.iter()
		.map(|quad| format!("{} .", quad.rdf_display()))
		.collect::<Vec<_>>()
		.join("\n")
		+ "\n"
}

#[cfg(test)]
mod tests {
	use std::fmt;

	use crate::sparql::rdf_type_conversions::IntoRdfTypes;
	use crate::sparql::{
		to_nquads, with_predicate, ConstructQuery, Join, SparqlQuery, ToConstructQuery,
	};
	use crate::{LinkedData, LinkedDataDeserializeSubject};
	use iref::IriBuf;
	use linked_data_derive::{Deserialize, Serialize};
	use oxigraph::sparql::QueryResults;
	use oxigraph::store::Store;
	use oxttl::NQuadsParser;
	use rdf_types::dataset::IndexedBTreeDataset;
	use rdf_types::generator::Blank;
	use rdf_types::interpretation::WithGenerator;
	use rdf_types::Generator;
	use spargebra::term::{NamedNode, Variable};

	#[derive(Serialize, Deserialize, Debug, PartialEq)]
	#[ld(prefix("ex" = "http://ex/"))]
	struct Id {
		#[ld(id)]
		id: IriBuf,

		#[ld("ex:field")]
		value: String,
	}

	#[derive(Serialize, Deserialize, Debug, Default, PartialEq)]
	#[ld(type = "http://ex/Type")]
	#[ld(prefix("ex" = "http://ex/"))]
	struct Type {
		#[ld("ex:field")]
		field: String,
	}

	#[derive(Serialize, Deserialize, Debug, PartialEq)]
	#[ld(prefix("ex" = "http://ex/"))]
	enum SimpleEnumType {
		#[ld("ex:left")]
		Left(String),
		#[ld("ex:right")]
		Right(String),
	}

	#[derive(Serialize, Deserialize, Debug, PartialEq)]
	#[ld(prefix("ex" = "http://ex/"))]
	enum SimpleEnum {
		#[ld("ex:left")]
		Left(String),
	}

	#[derive(Serialize, Deserialize, Debug, PartialEq)]
	#[ld(prefix("ex" = "http://ex/"))]
	enum Enum {
		#[ld("ex:left")]
		Left(String),
		#[ld("ex:right")]
		Right(Struct),
	}

	#[derive(Serialize, Deserialize, Debug, PartialEq)]
	#[ld(prefix("ex" = "http://ex/"))]
	struct Struct {
		#[ld("ex:field_0")]
		field_0: String,
		#[ld("ex:field_1")]
		field_1: String,
	}

	#[derive(Serialize, Deserialize, Debug, PartialEq)]
	#[ld(prefix("ex" = "http://ex/"))]
	struct FlattendStruct {
		#[ld("ex:field")]
		field: String,
		#[ld(flatten)]
		child: Struct,
	}

	#[derive(Serialize, Deserialize, Debug, PartialEq)]
	#[ld(prefix("ex" = "http://ex/"))]
	enum SimplePropertyCompoundEnum {
		#[ld("ex:left")]
		Left(#[ld("ex:value")] String),
	}

	#[derive(Serialize, Deserialize, Debug, PartialEq)]
	#[ld(prefix("ex" = "http://ex/"))]
	struct CrazyStruct {
		#[ld("ex:struct_id")]
		id: Id,
		#[ld("ex:struct_type")]
		type_field: Type,
		#[ld("ex:struct_flattened")]
		flattened: FlattendStruct,
	}

	#[derive(Serialize, Deserialize, Debug, PartialEq)]
	#[ld(prefix("ex" = "http://ex/"))]
	enum Crazy {
		#[ld("ex:enum_id")]
		Id(#[ld("ex:id")] Id),
		#[ld("ex:enum_typed")]
		Type(#[ld("ex:typed")] Type),
		#[ld("ex:enum_flat")]
		Flattend(#[ld("ex:flat")] FlattendStruct),
	}

	/// This will be generated
	impl ToConstructQuery for Id {
		fn to_query_with_binding(binding_variable: Variable) -> ConstructQuery {
			ConstructQuery::default()
				.join_with_binding(
					binding_variable.clone(),
					NamedNode::new_unchecked("http://ex/field"),
					String::to_query_with_binding,
				)
				.filter_variable(
					binding_variable.clone(),
					NamedNode::new_unchecked("http://example.org/myBar"),
				)
		}
	}

	impl ToConstructQuery for Type {
		fn to_query_with_binding(binding_variable: Variable) -> ConstructQuery {
			ConstructQuery::new_with_binding(
				binding_variable.clone(),
				NamedNode::new_unchecked("http://ex/field"),
				String::to_query_with_binding,
			)
			.join_with(
				binding_variable.clone(),
				NamedNode::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#type"),
				NamedNode::new_unchecked("http://ex/Type"),
			)
		}
	}

	impl ToConstructQuery for SimpleEnumType {
		fn to_query_with_binding(binding_variable: Variable) -> ConstructQuery {
			ConstructQuery::new_with_binding(
				binding_variable.clone(),
				NamedNode::new_unchecked("http://ex/left"),
				String::to_query_with_binding,
			)
			.union_with_binding(
				binding_variable.clone(),
				NamedNode::new_unchecked("http://ex/right"),
				String::to_query_with_binding,
			)
		}
	}

	impl ToConstructQuery for SimpleEnum {
		fn to_query_with_binding(binding_variable: Variable) -> ConstructQuery {
			ConstructQuery::new_with_binding(
				binding_variable.clone(),
				NamedNode::new_unchecked("http://ex/left"),
				String::to_query_with_binding,
			)
			.union_with_binding(
				binding_variable.clone(),
				NamedNode::new_unchecked("http://ex/right"),
				String::to_query_with_binding,
			)
		}
	}

	impl ToConstructQuery for SimplePropertyCompoundEnum {
		fn to_query_with_binding(binding_variable: Variable) -> ConstructQuery {
			ConstructQuery::new_with_binding(
				binding_variable.clone(),
				NamedNode::new_unchecked("http://ex/left"),
				with_predicate(
					NamedNode::new_unchecked("http://ex/value"),
					String::to_query_with_binding,
				),
			)
		}
	}

	impl ToConstructQuery for Enum {
		fn to_query_with_binding(binding_variable: Variable) -> ConstructQuery {
			ConstructQuery::new_with_binding(
				binding_variable.clone(),
				NamedNode::new_unchecked("http://ex/left"),
				String::to_query_with_binding,
			)
			.union_with_binding(
				binding_variable.clone(),
				NamedNode::new_unchecked("http://ex/right"),
				Struct::to_query_with_binding,
			)
		}
	}

	impl ToConstructQuery for Struct {
		fn to_query_with_binding(binding_variable: Variable) -> ConstructQuery {
			ConstructQuery::new_with_binding(
				binding_variable.clone(),
				NamedNode::new_unchecked("http://ex/field_0"),
				String::to_query_with_binding,
			)
			.join_with_binding(
				binding_variable.clone(),
				NamedNode::new_unchecked("http://ex/field_1"),
				String::to_query_with_binding,
			)
		}
	}

	impl ToConstructQuery for FlattendStruct {
		fn to_query_with_binding(binding_variable: Variable) -> ConstructQuery {
			ConstructQuery::default()
				.join_with_binding(
					binding_variable.clone(),
					NamedNode::new_unchecked("http://ex/field"),
					String::to_query_with_binding,
				)
				.join(Struct::to_query_with_binding(binding_variable.clone()))
		}
	}

	fn test_sparql<T>(expected: &T, id: Option<IriBuf>)
	where
		T: LinkedData<WithGenerator<Blank>>
			+ SparqlQuery
			+ LinkedDataDeserializeSubject
			+ PartialEq
			+ fmt::Debug,
	{
		let expected_nquads = to_nquads(expected).into_bytes();

		println!();
		println!();
		println!("Expected NQuads:");
		println!("{}", String::from_utf8(expected_nquads.clone()).unwrap());

		let quads = NQuadsParser::new().for_slice(&expected_nquads);
		let store = Store::new().unwrap();
		quads.filter_map(Result::ok).for_each(|quad| {
			store.insert(&quad).unwrap();
		});

		let query = expected.as_sparql_algebra();

		println!("Generated Query:");
		println!("{}", query);
		println!();
		println!("Generated SSE:");
		println!("{}", query.to_sse());

		let mut expected_dataset = IndexedBTreeDataset::new();
		println!();
		println!("Actual NQuads:");
		if let QueryResults::Graph(triples) = store.query(query).unwrap() {
			triples.filter_map(Result::ok).for_each(|triple| {
				let quad = triple.into_rdf_types();
				println!("{}", quad);
				expected_dataset.insert(quad);
			})
		}

		let subject = if let Some(iri) = id {
			<rdf_types::Term as rdf_types::FromIri>::from_iri(iri)
		} else {
			// Use a blank node as default
			Blank::new().next(&mut ()).into_term()
		};

		let actual = T::deserialize_subject(&(), &(), &expected_dataset, None, &subject).unwrap();

		assert_eq!(expected, &actual);
	}

	#[test]
	fn test_simple_enum() {
		let input = SimpleEnum::Left("left".to_owned());
		test_sparql(&input, None);
	}

	#[test]
	fn test_enum() {
		let simple_struct = Struct {
			field_0: "zero".to_owned(),
			field_1: "one".to_owned(),
		};
		let enum_ = Enum::Right(simple_struct);
		test_sparql(&enum_, None);
	}

	#[test]
	fn test_simple_struct() {
		let input = Struct {
			field_0: "zero".to_owned(),
			field_1: "one".to_owned(),
		};
		test_sparql(&input, None);
	}

	#[test]
	fn test_flattend_struct() {
		let child = Struct {
			field_0: "flattened zero".to_owned(),
			field_1: "flattened one".to_owned(),
		};
		let input = FlattendStruct {
			child,
			field: "parent".to_owned(),
		};
		test_sparql(&input, None);
	}

	#[test]
	fn test_simple_property_compound_enum() {
		let input = SimplePropertyCompoundEnum::Left("value".to_owned());
		test_sparql(&input, None);
	}

	#[test]
	fn test_id() {
		let id = IriBuf::new("http://example.org/myBar".to_string()).unwrap();
		let input = Id {
			id: id.clone(),
			value: "value".to_owned(),
		};
		test_sparql(&input, Some(id));
	}

	#[test]
	fn test_type() {
		let input = Type::default();
		test_sparql(&input, None);
	}

	#[test]
	fn test_simple_enum_type() {
		// TODO linked-data-rs does not add the type to enums
		let input = SimpleEnumType::Left("value".to_owned());
		test_sparql(&input, None);
	}
}

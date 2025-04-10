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
	use linked_data_derive::{Deserialize, Serialize, SparqlSerialize};
	use oxigraph::sparql::QueryResults;
	use oxigraph::store::Store;
	use oxttl::NQuadsParser;
	use rdf_types::dataset::IndexedBTreeDataset;
	use rdf_types::generator::Blank;
	use rdf_types::interpretation::WithGenerator;
	use rdf_types::Generator;
	use spargebra::term::{NamedNode, Variable};

	#[derive(SparqlSerialize, Serialize, Deserialize, Debug, PartialEq)]
	#[ld(prefix("ex" = "http://ex/"))]
	struct Struct {
		#[ld("ex:field_0")]
		field_0: String,

		#[ld("ex:field_1")]
		field_1: String,
	}

	/// This will be generated
	// impl ToConstructQuery for Struct {
	// 	fn to_query_with_binding(binding_variable: Variable) -> ConstructQuery {
	// 		ConstructQuery::default()
	// 			.join_with_binding(
	// 				binding_variable.clone(),
	// 				NamedNode::new_unchecked("http://ex/field_0"),
	// 				String::to_query_with_binding,
	// 			)
	// 			.join_with_binding(
	// 				binding_variable.clone(),
	// 				NamedNode::new_unchecked("http://ex/field_1"),
	// 				String::to_query_with_binding,
	// 			)
	// 	}
	// }

	#[derive(SparqlSerialize, Serialize, Deserialize, Debug, PartialEq)]
	#[ld(prefix("ex" = "http://ex/"))]
	struct StructId {
		#[ld(id)]
		id: IriBuf,

		#[ld("ex:field")]
		value: String,
	}

	/// This will be generated
	// impl ToConstructQuery for StructId {
	// 	fn to_query_with_binding(binding_variable: Variable) -> ConstructQuery {
	// 		ConstructQuery::default()
	// 			.join_with_binding(
	// 				binding_variable.clone(),
	// 				NamedNode::new_unchecked("http://ex/field"),
	// 				String::to_query_with_binding,
	// 			)
	// 			// NOTE Use this later
	// 			.filter_variable(
	// 				binding_variable.clone(),
	// 				NamedNode::new_unchecked("http://example.org/myBar"),
	// 			)
	// 	}
	// }

	#[derive(SparqlSerialize, Serialize, Deserialize, Debug, Default, PartialEq)]
	#[ld(type = "http://ex/Type")]
	#[ld(prefix("ex" = "http://ex/"))]
	struct StuctType {
		#[ld("ex:field")]
		field: String,
	}

	/// This will be generated
	// impl ToConstructQuery for StuctType {
	// 	fn to_query_with_binding(binding_variable: Variable) -> ConstructQuery {
	// 		ConstructQuery::default()
	// 			.join_with_binding(
	// 				binding_variable.clone(),
	// 				NamedNode::new_unchecked("http://ex/field"),
	// 				String::to_query_with_binding,
	// 			)
	// 			.join_with(
	// 				binding_variable.clone(),
	// 				NamedNode::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#type"),
	// 				NamedNode::new_unchecked("http://ex/Type"),
	// 			)
	// 	}
	// }

	#[derive(SparqlSerialize, Serialize, Deserialize, Debug, PartialEq)]
	#[ld(prefix("ex" = "http://ex/"))]
	struct StructFlatten {
		#[ld(flatten)]
		child: Struct,
	}

	/// This will be generated
	// impl ToConstructQuery for StructFlatten {
	// 	fn to_query_with_binding(binding_variable: Variable) -> ConstructQuery {
	// 		ConstructQuery::default().join(Struct::to_query_with_binding(binding_variable.clone()))
	// 	}
	// }

	#[derive(Serialize, Deserialize, Debug, PartialEq)]
	#[ld(prefix("ex" = "http://ex/"))]
	struct StructVec {
		#[ld("ex:vec")]
		more: Vec<Struct>,
	}

	#[derive(SparqlSerialize, Serialize, Deserialize, Debug, PartialEq)]
	#[ld(prefix("ex" = "http://ex/"))]
	enum Enum {
		#[ld("ex:left")]
		Left(String),

		#[ld("ex:right")]
		Right(Struct),
	}

	/// This will be generated
	// impl ToConstructQuery for Enum {
	// 	fn to_query_with_binding(binding_variable: Variable) -> ConstructQuery {
	// 		ConstructQuery::default()
	// 			.union_with_binding(
	// 				binding_variable.clone(),
	// 				NamedNode::new_unchecked("http://ex/left"),
	// 				String::to_query_with_binding,
	// 			)
	// 			.union_with_binding(
	// 				binding_variable.clone(),
	// 				NamedNode::new_unchecked("http://ex/right"),
	// 				Struct::to_query_with_binding,
	// 			)
	// 	}
	// }

	#[derive(Serialize, Deserialize, Debug, PartialEq)]
	#[ld(type = "http://ex/Type")]
	#[ld(prefix("ex" = "http://ex/"))]
	enum EnumType {
		#[ld(type = "http://ex/Type")]
		#[ld("ex:left")]
		Left(String),
	}

	#[derive(SparqlSerialize, Serialize, Deserialize, Debug, PartialEq)]
	#[ld(prefix("ex" = "http://ex/"))]
	enum EnumBlankNode {
		#[ld("ex:left")]
		Left(#[ld("ex:value")] String),
	}

	// impl ToConstructQuery for EnumBlankNode {
	// 	fn to_query_with_binding(binding_variable: Variable) -> ConstructQuery {
	// 		ConstructQuery::default().union_with_binding(
	// 			binding_variable.clone(),
	// 			NamedNode::new_unchecked("http://ex/left"),
	// 			with_predicate(
	// 				NamedNode::new_unchecked("http://ex/value"),
	// 				String::to_query_with_binding,
	// 			),
	// 		)
	// 	}
	// }

    // NOTE Nested id do not wirk
	#[derive(SparqlSerialize, Serialize, Deserialize, Debug, PartialEq)]
	#[ld(type = "http://ex/Type")]
	#[ld(prefix("ex" = "http://ex/"))]
	struct CrazyStruct {
		#[ld(id)]
		id: IriBuf,

		#[ld("ex:struct_id")]
		id_field: StructId,

		// #[ld("ex:struct_type")]
		// type_field: StuctType,
		//
		// #[ld("ex:struct_flatten")]
		// flatten_field: StructFlatten,
		//
		// #[ld("ex:enum_crazy")]
		// crazy_field: CrazyEnum,
	}

	#[derive(SparqlSerialize, Serialize, Deserialize, Debug, PartialEq)]
	#[ld(prefix("ex" = "http://ex/"))]
	enum CrazyEnum {
		#[ld("ex:enum_id")]
		Id(#[ld("ex:id")] StructId),

		#[ld("ex:enum_type")]
		Type(#[ld("ex:type")] StuctType),

		#[ld("ex:enum_flatten")]
		Flatten(#[ld("ex:flatten")] StructFlatten),
	}

	impl ToConstructQuery for EnumType {
		fn to_query_with_binding(binding_variable: Variable) -> ConstructQuery {
			ConstructQuery::new_with_binding(
				binding_variable.clone(),
				NamedNode::new_unchecked("http://ex/left"),
				String::to_query_with_binding,
			)
			.join_with(
				binding_variable.clone(),
				NamedNode::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#type"),
				NamedNode::new_unchecked("http://ex/Type"),
			)
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

	fn create_struct() -> Struct {
		Struct {
			field_0: "zero".to_owned(),
			field_1: "one".to_owned(),
		}
	}

	fn create_struct_id() -> StructId {
		let id = IriBuf::new("http://example.org/myBar".to_string()).unwrap();
		StructId {
			id,
			value: "value".to_owned(),
		}
	}

	fn create_struct_type() -> StuctType {
		StuctType {
			field: "type_field".to_owned(),
		}
	}

	fn create_struct_flatten() -> StructFlatten {
		StructFlatten {
			child: create_struct(),
		}
	}

	fn create_struct_vec() -> StructVec {
		StructVec {
			more: vec![
				create_struct(),
				Struct {
					field_0: "item2-zero".to_owned(),
					field_1: "item2-one".to_owned(),
				},
			],
		}
	}

	fn create_enum() -> Enum {
		Enum::Right(create_struct())
	}

	fn create_enum_type() -> EnumType {
		EnumType::Left("left".to_owned())
	}

	fn create_enum_blank_node() -> EnumBlankNode {
		EnumBlankNode::Left("value".to_owned())
	}

	fn create_crazy_struct() -> CrazyStruct {
		let id = IriBuf::new("http://example.org/crazy".to_string()).unwrap();
		CrazyStruct {
			id,
			id_field: create_struct_id(),
			// type_field: create_struct_type(),
			// flatten_field: create_struct_flatten(),
			// crazy_field: create_crazy_enum(),
		}
	}

	fn create_crazy_enum() -> CrazyEnum {
		CrazyEnum::Id(create_struct_id())
	}

	#[test]
	fn test_struct() {
		test_sparql(&create_struct(), None);
	}

	#[test]
	fn test_struct_id() {
		let struct_id = create_struct_id();
		test_sparql(&struct_id, Some(struct_id.id.clone()));
	}

	#[test]
	fn test_struct_type() {
		test_sparql(&create_struct_type(), None);
	}

	#[test]
	fn test_struct_flatten() {
		test_sparql(&create_struct_flatten(), None);
	}

	// NOTE Deserialize missing
	#[test]
	#[ignore]
	fn test_struct_vec() {}

	// NOTE Deserialize missing
	#[test]
	#[ignore]
	fn test_struct_graph() {}

	#[test]
	fn test_enum() {
		test_sparql(&create_enum(), None);
	}
	// NOTE Type attribute for enum missing
	#[test]
	#[ignore]
	fn test_enum_type() {
		test_sparql(&create_enum_type(), None);
	}

	#[test]
	fn test_enum_blank_node() {
		test_sparql(&create_enum_blank_node(), None);
	}

	#[test]
	fn test_crazy_struct() {
		test_sparql(&create_crazy_struct(), None);
	}
}

use rdf_types::generator::Blank;
use rdf_types::interpretation::WithGenerator;
use rdf_types::RdfDisplay;
use spargebra::algebra::GraphPattern;
use spargebra::term::{NamedNode, NamedNodePattern, TermPattern, TriplePattern, Variable};
use spargebra::Query;
use sparopt::Optimizer;
use uuid::Uuid;

use crate::{to_quads_with, LinkedData};

#[derive(Default)]
struct ConstructQuery {
	template: Vec<TriplePattern>,
	pattern: GraphPattern,
}

pub trait Sparql {
	fn to_sparql() -> String {
		Self::to_sparql_algebra().to_string()
	}

	fn as_sparql(&self) -> String {
		self.as_sparql_algebra().to_string()
	}

	fn to_sparql_algebra() -> Query;

	fn as_sparql_algebra(&self) -> Query {
		Self::to_sparql_algebra()
	}
}

trait ToQuery {
	fn to_bound_query(binding_variable: Variable) -> ConstructQuery;

	fn to_query() -> ConstructQuery {
		Self::to_bound_query(generate_variable())
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
			template: patterns.clone(),
			pattern: GraphPattern::Bgp { patterns },
		}
	}

	fn new_with<F>(subject: Variable, predicate: NamedNode, to_bound_query: F) -> Self
	where
		F: FnOnce(Variable) -> Self,
	{
		let object = generate_variable();
		ConstructQuery::new(subject, predicate, object.clone()).join(to_bound_query(object))
	}

	fn union_with<F>(self, subject: Variable, predicate: NamedNode, to_bound_query: F) -> Self
	where
		F: FnOnce(Variable) -> Self,
	{
		let object = generate_variable();
		self.union(ConstructQuery::new(subject, predicate, object.clone()))
			.join(to_bound_query(object))
	}

	fn join_with<F>(self, subject: Variable, predicate: NamedNode, to_bound_query: F) -> Self
	where
		F: FnOnce(Variable) -> Self,
	{
		let object = generate_variable();
		self.join(ConstructQuery::new(subject, predicate, object.clone()))
			.join(to_bound_query(object))
	}
}

impl From<ConstructQuery> for Query {
	fn from(value: ConstructQuery) -> Self {
		// TODO Remove the optimizer
		let pattern = (&Optimizer::optimize_graph_pattern((&value.pattern).into())).into();
		Query::Construct {
			template: value.template,
			dataset: None,
			pattern,
			base_iri: None,
		}
	}
}

impl Join for ConstructQuery {
	fn join(mut self, other: Self) -> Self {
		self.template = self.template.and(other.template);
		self.pattern = self.pattern.join(other.pattern);
		self
	}
}

impl Union for ConstructQuery {
	fn union(mut self, other: Self) -> Self {
		self.template = self.template.and(other.template);
		self.pattern = self.pattern.union(other.pattern);
		self
	}
}

impl<T> Sparql for T
where
	T: ToQuery,
{
	fn to_sparql_algebra() -> Query {
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

impl ToQuery for String {
	fn to_bound_query(_: Variable) -> ConstructQuery {
		ConstructQuery::default()
	}
}

fn generate_variable() -> Variable {
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
	use crate::sparql::{to_nquads, ConstructQuery, Sparql, ToQuery};
	use crate::LinkedData;
	use linked_data_derive::{Deserialize, Serialize};
	use oxigraph::io::RdfFormat;
	use oxigraph::store::Store;
	use oxttl::NQuadsParser;
	use rdf_types::generator::Blank;
	use rdf_types::interpretation::WithGenerator;
	use spargebra::term::{NamedNode, Variable};

	#[derive(Serialize, Deserialize)]
	#[ld(prefix("ex" = "http://ex/"))]
	enum SimpleEnum {
		#[ld("ex:left")]
		Left(String),
		#[ld("ex:right")]
		Right(String),
	}

	#[derive(Serialize, Deserialize)]
	#[ld(prefix("ex" = "http://ex/"))]
	enum Enum {
		#[ld("ex:left")]
		Left(String),
		#[ld("ex:right")]
		Right(SimpleStruct),
	}

	#[derive(Serialize, Deserialize)]
	#[ld(prefix("ex" = "http://ex/"))]
	struct SimpleStruct {
		#[ld("ex:field_0")]
		field_0: String,
		#[ld("ex:field_1")]
		field_1: String,
	}

	/// This will be generated
	impl ToQuery for SimpleEnum {
		fn to_bound_query(binding_variable: Variable) -> ConstructQuery {
			ConstructQuery::new_with(
				binding_variable.clone(),
				NamedNode::new_unchecked("http://ex/left"),
				String::to_bound_query,
			)
			.union_with(
				binding_variable.clone(),
				NamedNode::new_unchecked("http://ex/right"),
				String::to_bound_query,
			)
		}
	}

	impl ToQuery for Enum {
		fn to_bound_query(binding_variable: Variable) -> ConstructQuery {
			ConstructQuery::new_with(
				binding_variable.clone(),
				NamedNode::new_unchecked("http://ex/left"),
				String::to_bound_query,
			)
			.union_with(
				binding_variable.clone(),
				NamedNode::new_unchecked("http://ex/right"),
				SimpleStruct::to_bound_query,
			)
		}
	}

	impl ToQuery for SimpleStruct {
		fn to_bound_query(binding_variable: Variable) -> ConstructQuery {
			ConstructQuery::new_with(
				binding_variable.clone(),
				NamedNode::new_unchecked("http://ex/field_0"),
				String::to_bound_query,
			)
			.join_with(
				binding_variable.clone(),
				NamedNode::new_unchecked("http://ex/field_1"),
				String::to_bound_query,
			)
		}
	}

	fn test_sparql<T: LinkedData<WithGenerator<Blank>> + Sparql>(input: &T) {
		let expected_nquads = to_nquads(input).into_bytes();

		println!();
		println!("Expected NQuads:");
		println!("{}", String::from_utf8(expected_nquads.clone()).unwrap());

		let quads = NQuadsParser::new().for_slice(&expected_nquads);
		let store = Store::new().unwrap();
		quads.filter_map(Result::ok).for_each(|quad| {
			store.insert(&quad).unwrap();
		});

		let query = input.as_sparql_algebra();

		println!("Generated Query:");
		println!("{}", query);
		println!();
		println!("Generated SSE:");
		println!("{}", query.to_sse());

		let result = store.query(query).unwrap();
		let actual_nquads = result.write_graph(Vec::new(), RdfFormat::NQuads).unwrap();

		println!();
		println!("Actual NQuads:");
		println!("{}", String::from_utf8(actual_nquads.clone()).unwrap());

		// assert_eq!(expected_nquads, actual_nquads);
	}

	// #[test]
	fn test_simple_enum() {
		let input = SimpleEnum::Left("left".to_owned());
		test_sparql(&input);
	}

	#[test]
	fn test_enum() {
		let simple_struct = SimpleStruct {
			field_0: "zero".to_owned(),
			field_1: "one".to_owned(),
		};
		let enum_ = Enum::Right(simple_struct);
		test_sparql(&enum_);
	}

	// #[test]
	fn test_simple_struct() {
		let input = SimpleStruct {
			field_0: "zero".to_owned(),
			field_1: "one".to_owned(),
		};
		test_sparql(&input);
	}
}

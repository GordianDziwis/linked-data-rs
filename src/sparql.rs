use spargebra::term::{Literal, NamedNode, NamedNodePattern, TermPattern, TriplePattern, Variable};
use spargebra::Query;
use uuid::Uuid;

pub trait ToSparql {
	fn to_sparql() -> String {
		Self::to_sparql_algebra().to_string()
	}

	fn to_sparql_algebra() -> Query;
}

impl<T> ToSparql for T
where
	T: ToTriplePattern,
{
	fn to_sparql_algebra() -> Query {
		Query::Construct {
			template: Self::to_triple_pattern(),
			dataset: None,
			pattern: spargebra::algebra::GraphPattern::Bgp {
				patterns: Self::to_triple_pattern(),
			},
			base_iri: None,
		}
	}
}

trait ToTriplePattern {
	fn to_bound_triple_pattern(binding_variable: Variable) -> Vec<TriplePattern>;

	fn to_triple_pattern() -> Vec<TriplePattern> {
		Self::to_bound_triple_pattern(generate_variable())
	}
}

impl ToTriplePattern for String {
	fn to_bound_triple_pattern(_: Variable) -> Vec<TriplePattern> {
		vec![]
	}
}

fn triple_pattern(
	subject: impl Into<TermPattern>,
	predicate: impl Into<NamedNodePattern>,
	object: impl Into<TermPattern>,
) -> Vec<TriplePattern> {
	vec![TriplePattern {
		subject: subject.into(),
		predicate: predicate.into(),
		object: object.into(),
	}]
}

fn generate_variable() -> Variable {
	let variable = format!("{}", Uuid::new_v4().simple());
	Variable::new(variable).expect("XXX")
}

trait BindPattern {
	fn bind_pattern<F>(
		&mut self,
		subject: Variable,
		predicate: NamedNode,
		to_bound_triple_pattern: F,
	) where
		Self: Sized + Extend<TriplePattern>,
		F: FnOnce(Variable) -> Vec<TriplePattern>,
	{
		let object = generate_variable();
		self.extend(triple_pattern(subject, predicate, object.clone()));
		self.extend(to_bound_triple_pattern(object));
	}
}

impl BindPattern for Vec<TriplePattern> {}

#[cfg(test)]
mod tests {
	use crate::sparql::{BindPattern, ToSparql, ToTriplePattern};
	use linked_data_derive::SparqlSerialize;
	use spargebra::term::{NamedNode, TriplePattern, Variable};

	#[derive(SparqlSerialize)]
	#[ld(prefix("ex" = "http://ex/"))]
	struct SimplePattern {
		#[ld("ex:field_0")]
		field_0: String,
		#[ld("ex:field_1")]
		field_1: String,
	}

    /// This will be generated
	impl ToTriplePattern for SimplePattern {
		fn to_bound_triple_pattern(binding_variable: Variable) -> Vec<TriplePattern> {
			let mut pattern: Vec<TriplePattern> = vec![];

			pattern.bind_pattern(
				binding_variable.clone(),
				NamedNode::new("http://ex/field_0").unwrap(),
				String::to_bound_triple_pattern,
			);

			pattern.bind_pattern(
				binding_variable.clone(),
				NamedNode::new("http://ex/field_1").unwrap(),
				String::to_bound_triple_pattern,
			);

			pattern
		}
	}

	#[test]
	fn test_simple_pattern() {
		assert_eq!(SimplePattern::to_sparql(), "")
	}
}

// trait VariableGenerator {
// 	fn next(&mut self) -> Variable;
// }
//
// struct NumberedVariableGenerator(usize);
//
// impl NumberedVariableGenerator {
// 	fn new() -> Self {
// 		NumberedVariableGenerator(0)
// 	}
// }
//
// impl VariableGenerator for NumberedVariableGenerator {
// 	/// # Panics
// 	///
// 	/// Panics when the generated number exceeds usize,
// 	fn next(&mut self) -> Variable {
// 		self.0 += 1;
// 		Variable::new(self.0.to_string()).expect("XXX")
// 	}
// }

// enum TripleOrTermPattern {
// 	TriplePattern(Vec<TriplePattern>),
// 	TermPattern(TermPattern),
// }
//
// impl From<Vec<TriplePattern>> for TripleOrTermPattern {
// 	fn from(value: Vec<TriplePattern>) -> Self {
// 		Self::TriplePattern(value)
// 	}
// }
//
// impl From<TermPattern> for TripleOrTermPattern {
// 	fn from(value: TermPattern) -> Self {
// 		Self::TermPattern(value)
// 	}
// }
//
// impl From<Literal> for TripleOrTermPattern {
// 	fn from(value: Literal) -> Self {
// 		Self::TermPattern(value.into())
// 	}
// }
//
// impl From<Variable> for TripleOrTermPattern {
// 	fn from(value: Variable) -> Self {
// 		Self::TermPattern(value.into())
// 	}
// }

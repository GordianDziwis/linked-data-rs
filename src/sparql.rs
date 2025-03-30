use crate::sparql::interpretation::SparqlInterpretation;
use crate::{to_interpreted_quads, LinkedData};
use rdf_types::{Quad, Triple};
pub mod interpretation;
// pub mod domain;

pub trait Sparql {
	fn get_sparql() -> String;

	// fn get_sparql_query_builder() -> BookQueryBuilder;
}

impl<T> Sparql for T
where
	T: Default + LinkedData<SparqlInterpretation>,
{
	fn get_sparql() -> String {
		to_sparql(&Self::default())
	}

	// fn get_sparql_query_builder() -> BookQueryBuilder {
	// 	BookQueryBuilder::default()
	// }
}

pub fn to_sparql(value: &impl LinkedData<SparqlInterpretation>) -> String {
	let mut interpretation = SparqlInterpretation::default();

	let pattern_lines: Vec<String> = to_interpreted_quads(&mut (), &mut interpretation, value)
		.expect("RDF serialization failed")
		.into_iter()
		.map(|Quad(s, p, o, _)| Triple(s, p, o))
		.map(|triple| format!("    {} .", triple))
		.collect();

	let pattern = pattern_lines.join("\n");

	format!("CONSTRUCT {{\n{}\n}}\nWHERE {{\n{}\n}}", pattern, pattern)
}

use std::collections::HashMap;

use iref::IriBuf;
use rdf_types::generator::Blank;
use rdf_types::interpretation::{
	BlankIdInterpretationMut, IriInterpretationMut, LiteralInterpretationMut,
};
use rdf_types::vocabulary::{BlankIdVocabularyMut, IriVocabularyMut};
use rdf_types::{
	BlankIdBuf, FromBlankId, FromIri, Generator, Interpretation, InterpretationMut, Literal,
	RdfDisplay, Term, Vocabulary,
};

#[derive(Default, Debug)]
pub struct SparqlInterpretation<G: Generator = Blank> {
	pub generator: G,
	pub variables: HashMap<Term, VarOrTerm>,
	pub next_var_index: usize,
}

#[derive(Clone, Debug)]
pub enum VarOrTerm {
	Var(SparqlVariable),
	Term(Term),
}

#[derive(Clone, Debug, Default)]
pub struct SparqlVariable(pub String);

impl SparqlInterpretation {
	pub fn add_term(&mut self, term: Term) -> <Self as Interpretation>::Resource {
		match self.variables.get(&term) {
			Some(sparql_variable) => sparql_variable.clone(),
			None => {
				let sparql_variable = self.new_resource(&mut ());
				self.variables.insert(term, sparql_variable.clone());
				sparql_variable
			}
		}
	}
}

impl RdfDisplay for VarOrTerm {
	fn rdf_fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		match self {
			VarOrTerm::Var(sparql_variable) => sparql_variable.rdf_fmt(f),
			VarOrTerm::Term(term) => term.rdf_fmt(f),
		}
	}
}

impl RdfDisplay for SparqlVariable {
	fn rdf_fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		write!(f, "{}", self.0)
	}
}

impl Interpretation for SparqlInterpretation {
	type Resource = VarOrTerm;
}

impl<V> InterpretationMut<V> for SparqlInterpretation
where
	V: Vocabulary + BlankIdVocabularyMut<BlankId = BlankIdBuf> + IriVocabularyMut<Iri = IriBuf>,
{
	fn new_resource(&mut self, vocabulary: &mut V) -> Self::Resource {
		// TODO Case index > 26
		let letter = (b'a' + (self.next_var_index as u8 % 26)) as char;
		self.next_var_index += 1;
		let term = self.generator.next(vocabulary);
		let sparql_variable = VarOrTerm::Var(SparqlVariable(format!("?{}", letter)));
		self.variables
			.insert(term.into_term(), sparql_variable.clone());
		sparql_variable
	}
}

impl BlankIdInterpretationMut for SparqlInterpretation {
	fn interpret_blank_id(&mut self, blank_id: BlankIdBuf) -> Self::Resource {
		let term = Term::from_blank(blank_id);
		self.variables
			.get(&term)
			.unwrap_or(&VarOrTerm::Term(term))
			.clone()
	}
}

impl IriInterpretationMut for SparqlInterpretation {
	fn interpret_iri(&mut self, iri: IriBuf) -> Self::Resource {
		let term = Term::from_iri(iri);
		self.variables
			.get(&term)
			.unwrap_or(&VarOrTerm::Term(term))
			.clone()
	}
}

impl LiteralInterpretationMut for SparqlInterpretation {
	fn interpret_literal(&mut self, _literal: Literal) -> Self::Resource {
		self.new_resource(&mut ())
	}
}

use std::str::FromStr;

pub trait IntoRdfTypes: Sized {
	type T;
	fn into_rdf_types(self) -> Self::T;
}

impl IntoRdfTypes for spargebra::term::Term {
	type T = rdf_types::Term;

	fn into_rdf_types(self) -> Self::T {
		match self {
			spargebra::term::Term::NamedNode(id) => {
				rdf_types::Term::Id(rdf_types::Id::Iri(id.into_rdf_types()))
			}
			spargebra::term::Term::BlankNode(blank_node) => {
				rdf_types::Term::Id(rdf_types::Id::Blank(blank_node.into_rdf_types()))
			}
			spargebra::term::Term::Literal(literal) => {
				rdf_types::Term::Literal(literal.into_rdf_types())
			}
			spargebra::term::Term::Triple(_) => panic!(),
		}
	}
}

impl IntoRdfTypes for spargebra::term::Literal {
	type T = rdf_types::Literal;

	fn into_rdf_types(self) -> Self::T {
		match self.destruct() {
			(value, Some(datatype_iri), None) => rdf_types::Literal::new(
				value,
				rdf_types::LiteralType::Any(datatype_iri.into_rdf_types()),
			),
			(value, None, Some(language_tag)) => rdf_types::Literal::new(
				value,
				rdf_types::LiteralType::LangString(
					langtag::LangTagBuf::from_str(&language_tag).unwrap(),
				),
			),
			(value, None, None) => rdf_types::Literal::new(
				value,
				rdf_types::LiteralType::Any(rdf_types::XSD_STRING.to_owned()),
			),
			_ => panic!(),
		}
	}
}

impl IntoRdfTypes for spargebra::term::NamedNode {
	type T = rdf_types::IriBuf;

	fn into_rdf_types(self) -> Self::T {
		rdf_types::IriBuf::try_from(self.into_string()).unwrap()
	}
}

impl IntoRdfTypes for spargebra::term::BlankNode {
	type T = rdf_types::BlankIdBuf;

	fn into_rdf_types(self) -> Self::T {
		rdf_types::BlankIdBuf::new(format!("{}", self)).unwrap()
	}
}

impl IntoRdfTypes for spargebra::term::Subject {
	type T = rdf_types::Id;

	fn into_rdf_types(self) -> Self::T {
		match self {
			spargebra::term::Subject::NamedNode(id) => rdf_types::Id::Iri(id.into_rdf_types()),
			spargebra::term::Subject::BlankNode(blank_node) => {
				rdf_types::Id::Blank(blank_node.into_rdf_types())
			}
			spargebra::term::Subject::Triple(_) => panic!(),
		}
	}
}

impl IntoRdfTypes for spargebra::term::Triple {
	type T = rdf_types::Quad;

	fn into_rdf_types(self) -> Self::T {
		rdf_types::Quad(
			self.subject.into_rdf_types().into_term(),
			rdf_types::Id::Iri(self.predicate.into_rdf_types()).into_term(),
			self.object.into_rdf_types(),
			None,
		)
	}
}

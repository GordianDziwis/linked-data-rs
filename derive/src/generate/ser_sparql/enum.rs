use std::collections::HashMap;

use iref::IriBuf;
use proc_macro2::{Ident, TokenStream};
use proc_macro_error::abort;
use quote::ToTokens;
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{Attribute, DataEnum, Field, Type, Variant};

use crate::generate::{read_variant_attributes, TypeAttributes, VariantAttributes};

use super::Error;

struct SparqlEnum<'p, 'ast> {
	ident: &'ast Ident,
	prefixes: &'p HashMap<String, String>,
	variants: Vec<SparqlVariant<'ast>>,
}

#[derive(Debug)]
struct SparqlVariant<'ast> {
	iri_attributes: SparqlIriAttributes,
	ty: &'ast Type,
}

#[derive(Debug)]
enum SparqlIriAttributes {
	Both {
		inner_iri: IriBuf,
		outer_iri: IriBuf,
	},
	One(IriBuf),
}

impl<'ast> SparqlVariant<'ast> {
	fn from_variant(variant: &'ast Variant, prefixes: &HashMap<String, String>) -> Self {
		let outer_iri = Self::extract_iri(variant.attrs.clone(), prefixes);
		let mut fields = variant.fields.iter();

		let Some(field) = fields.next() else {
			abort!(variant.fields.span(), "xx");
		};

		let inner_iri = Self::extract_iri(field.attrs.clone(), prefixes);

		if let Some(field) = fields.next() {
			abort!(field.span(), "Additional field detected");
		}

		let iri_attributes = SparqlIriAttributes::try_new(inner_iri, outer_iri)
			.unwrap_or_else(|error| abort!(variant.span(), error));

		SparqlVariant {
			iri_attributes,
			ty: &field.ty,
		}
	}

	fn extract_iri(
		attribute: Vec<Attribute>,
		prefixes: &HashMap<String, String>,
	) -> Option<IriBuf> {
		match read_variant_attributes(attribute) {
			Ok(VariantAttributes {
				iri: Some(compact_iri),
			}) => compact_iri
				.expand(prefixes)
				.map_err(|e| abort!(e.span(), e))
				.ok(),
			Ok(VariantAttributes { iri: None }) => None,
			Err(e) => abort!(e.span(), e),
		}
	}
}

impl SparqlIriAttributes {
	fn try_new(
		inner_iri: Option<IriBuf>,
		outer_iri: Option<IriBuf>,
	) -> Result<SparqlIriAttributes, String> {
		match (inner_iri, outer_iri) {
			(None, None) => Err("Variant witout IRI Attribute".to_owned()),
			(None, Some(outer_iri)) => Ok(SparqlIriAttributes::One(outer_iri)),
			(Some(inner_iri), None) => Ok(SparqlIriAttributes::One(inner_iri)),
			(Some(inner_iri), Some(outer_iri)) => Ok(SparqlIriAttributes::Both {
				inner_iri,
				outer_iri,
			}),
		}
	}
}

impl ToTokens for SparqlEnum<'_, '_> {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		let variants = &self.variants;
		let ident = self.ident;

		tokens.extend(quote::quote! {
			impl ToConstructQuery for #ident {
				fn to_query_with_binding(binding_variable: Variable) -> ConstructQuery {
					ConstructQuery::default()
					 #(#variants)*
				}
			}
		});
	}
}

impl ToTokens for SparqlVariant<'_> {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		let ty = self.ty;
		let inner_generator = quote::quote! { #ty::to_query_with_binding };
		println!("{:?}", &self.iri_attributes);

		let (iri_str, predicate_generator) = match &self.iri_attributes {
			SparqlIriAttributes::One(iri) => (iri.as_str(), inner_generator),
			SparqlIriAttributes::Both {
				inner_iri,
				outer_iri,
			} => {
				let inner_iri_str = inner_iri.as_str();
				(
					outer_iri.as_str(),
					quote::quote! {
						with_predicate(
							NamedNode::new_unchecked(#inner_iri_str),
							#inner_generator
						)
					},
				)
			}
		};

		tokens.extend(quote::quote! {
			.union_with_binding(
				binding_variable.clone(),
				NamedNode::new_unchecked(#iri_str),
				#predicate_generator
			)
		});
	}
}

impl<'ast> Visit<'ast> for SparqlEnum<'_, 'ast> {
	fn visit_variant(&mut self, i: &'ast Variant) {
		let sparql_variant = SparqlVariant::from_variant(i, self.prefixes);
		self.variants.push(sparql_variant);
	}
}
pub fn generate(
	attrs: &TypeAttributes,
	ident: Ident,
	generics: syn::Generics,
	e: syn::DataEnum,
) -> Result<TokenStream, Error> {
	let mut visitor = SparqlEnum {
		ident: &ident,
		prefixes: &attrs.prefixes,
		variants: vec![],
	};
	visitor.visit_data_enum(&e);

	let tokens = quote::quote!(#visitor);
	Ok(tokens)
}
//
// fn handle_variants(
// 	variants: Punctuated<Variant, Comma>,
// 	prefixes: &HashMap<String, String>,
// ) -> Result<TokenStream, Error> {
// 	variants
// 		.into_iter()
// 		.map(|variant| handle_variant(&variant, prefixes))
// 		.collect()
// }
//
// fn handle_variant(
// 	variant: &Variant,
// 	prefixes: &HashMap<String, String>,
// ) -> Result<TokenStream, Error> {
// 	let attributes = read_variant_attributes(variant.attrs.clone())?;
//
// 	let token_stream = TokenStream::new();
//
// 	if variant.fields.len() == 1 {
// 		match variant.fields.iter().next() {
// 			Some(x) => token_stream.extend(handle_iri(attributes.iri, variant, prefixes)?),
// 			None => todo!(),
// 		}
// 	}
//
// 	let token_stream = [handle_iri(attributes.iri, variant, prefixes)?]
// 		.into_iter()
// 		.fold(quote!(), |acc, tokens| quote! { #acc #tokens });
//
// 	Ok(token_stream)
// }
//
// fn handle_iri(
// 	iri: Option<CompactIri>,
// 	variant: &Type,
// 	prefixes: &HashMap<String, String>,
// ) -> Result<TokenStream, Error> {
// 	// let fields = read_variant_attribute();
// 	match iri {
// 		Some(iri) => {
// 			let expanded_iri = iri.expand(prefixes)?.into_string();
// 			// let type = variant.fields
// 			Ok(quote! {
// 				.join_with_binding(
// 					binding_variable.clone(),
// 					NamedNode::new_unchecked(#expanded_iri),
// 					String::to_query_with_binding,
// 				)
// 			})
// 		}
// 		None => Ok(TokenStream::new()),
// 	}
// }

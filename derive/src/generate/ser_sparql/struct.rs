use std::collections::HashMap;

use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::{Field, Fields};

use crate::generate::{read_field_attributes, CompactIri, TypeAttributes};

use super::Error;

pub fn generate(
	attrs: &TypeAttributes,
	ident: Ident,
	generics: syn::Generics,
	s: syn::DataStruct,
) -> Result<TokenStream, Error> {
	let fields = handle_fields(s.fields, &attrs.prefixes)?;
	let type_attribute = type_attribute(attrs)?;
	Ok(quote! {
		impl ToConstructQuery for #ident {
			fn to_query_with_binding(binding_variable: Variable) -> ConstructQuery {
				ConstructQuery::default()
				#fields
				#type_attribute
			}
		}
	})
}

fn type_attribute(attributes: &TypeAttributes) -> Result<TokenStream, Error> {
	if let Some(type_iri) = &attributes.type_ {
		let expanded_type_iri = type_iri.expand(&attributes.prefixes)?.into_string();
		Ok(quote! {
			.join_with(
				binding_variable.clone(),
				// TODO const for type
				NamedNode::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#type"),
				NamedNode::new_unchecked(#expanded_type_iri),
			)
		})
	} else {
		Ok(quote!())
	}
}

fn handle_fields(fields: Fields, prefixes: &HashMap<String, String>) -> Result<TokenStream, Error> {
	fields
		.into_iter()
		.map(|field| handle_field(field, prefixes))
		.collect()
}

fn handle_field(field: Field, prefixes: &HashMap<String, String>) -> Result<TokenStream, Error> {
	let attributes = read_field_attributes(field.attrs)?;

	if attributes.ignore {
		return Ok(quote!());
	}

	let token_stream = [
		handle_iri(attributes.iri, &field.ty, prefixes)?,
		handle_flatten(attributes.flatten, &field.ty)?,
	]
	.into_iter()
	.fold(quote!(), |acc, tokens| quote! { #acc #tokens });

	Ok(token_stream)
}

fn handle_flatten(flatten: bool, ty: &syn::Type) -> Result<TokenStream, Error> {
	match flatten {
		true => Ok(quote! {
			.join(#ty::to_query_with_binding(binding_variable.clone()))
		}),
		false => Ok(TokenStream::new()),
	}
}

fn handle_iri(
	iri: Option<CompactIri>,
	ty: &syn::Type,
	prefixes: &HashMap<String, String>,
) -> Result<TokenStream, Error> {
	match iri {
		Some(iri) => {
			let expanded_iri = iri.expand(prefixes)?.into_string();
			Ok(quote! {
				.join_with_binding(
					binding_variable.clone(),
					NamedNode::new_unchecked(#expanded_iri),
					#ty::to_query_with_binding,
				)
			})
		}
		None => Ok(TokenStream::new()),
	}
}

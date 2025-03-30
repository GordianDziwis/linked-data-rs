use linked_data::{Deserialize, Serialize, Sparql};

#[derive(Serialize, Deserialize, Default)]
#[ld(prefix("ex" = "http://example.org/"))]
struct Book {
	#[ld("dc:title")]
	title: String,
	#[ld("dc:author")]
	author: Author,
}

#[derive(Serialize, Deserialize, Default)]
#[ld(prefix("ex" = "http://example.org/"))]
struct Author {
	#[ld("ex:first")]
	title: String,
	#[ld("ex:second")]
	author: String,
}

#[derive(Serialize, Deserialize, Default)]
#[ld(prefix("ex" = "http://example.org/"))]
struct Foo {
	#[ld("ex:name")]
	name: String,

	#[ld(flatten)]
	more: MoreFoo,
}

#[derive(Serialize, Deserialize, Default)]
#[ld(prefix("ex" = "http://example.org/"))]
struct MoreFoo {
	#[ld("ex:email")]
	email: String,
}

fn main() {
	println!("{}", Book::get_sparql());
	println!("{}", Foo::get_sparql());
}

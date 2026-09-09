//! The `Table` derive for `iter`. It lives in its own crate only because
//! Rust requires that of a procedural macro; nothing else depends on it.

use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, LitStr, parse_macro_input};

/// Implements `crate::db::Table` for a struct, mapping it to a SQLite table.
///
/// The columns are the struct's own fields, in declaration order, so there
/// is no column list to keep in step with anything. A field named `id` is
/// the primary key: it's left out of the columns (SQLite assigns it on
/// insert, and it's the key rather than a payload on update) and read from
/// column 0 of every row. Every other field's type must implement
/// `crate::db::Column`, which is where the SQL conversions live.
///
/// `order_by` is optional, and supplies the trailing clause for a bare
/// `Repository::list`; without it the listing order is SQLite's.
///
/// ```ignore
/// #[derive(Table)]
/// #[table(name = "projects", order_by = "name")]
/// pub struct Project {
///     pub id: Option<i64>,
///     pub name: String,
/// }
/// ```
#[proc_macro_derive(Table, attributes(table))]
pub fn derive_table(input: TokenStream) -> TokenStream {
    expand(parse_macro_input!(input as DeriveInput))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

fn expand(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let ty = &input.ident;

    // #[table(name = "...", order_by = "...")]
    let attr = input
        .attrs
        .iter()
        .find(|attr| attr.path().is_ident("table"))
        .ok_or_else(|| {
            syn::Error::new(ty.span(), r#"missing `#[table(name = "...")]` attribute"#)
        })?;
    let (mut name, mut order_by) = (None, None);
    attr.parse_nested_meta(|meta| {
        let target = if meta.path.is_ident("name") {
            &mut name
        } else if meta.path.is_ident("order_by") {
            &mut order_by
        } else {
            return Err(meta.error("expected `name` or `order_by`"));
        };
        *target = Some(meta.value()?.parse::<LitStr>()?);
        Ok(())
    })?;
    let name = name.ok_or_else(|| {
        syn::Error::new_spanned(attr, r#"`#[table(...)]` needs `name = "<sql table>"`"#)
    })?;
    let list_tail = order_by.map_or(String::new(), |by| format!("ORDER BY {}", by.value()));

    // The columns are the struct's fields, minus the `id` primary key.
    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new(ty.span(), "`Table` needs a struct"));
    };
    let Fields::Named(fields) = &data.fields else {
        return Err(syn::Error::new(
            ty.span(),
            "`Table` needs named fields: each one maps to a column",
        ));
    };
    let columns: Vec<_> = fields
        .named
        .iter()
        .filter_map(|field| field.ident.as_ref())
        .filter(|field| *field != "id")
        .collect();
    if columns.is_empty() {
        return Err(syn::Error::new(
            ty.span(),
            "`Table` needs at least one field besides `id` to use as a column",
        ));
    }
    let column_names = columns.iter().map(|field| field.to_string());

    Ok(quote! {
        impl crate::db::Table for #ty {
            const NAME: &'static str = #name;
            const COLUMNS: &'static [&'static str] = &[#(#column_names),*];
            const LIST_TAIL: &'static str = #list_tail;

            fn from_row(row: &::rusqlite::Row) -> ::rusqlite::Result<Self> {
                // Column 0 is `id`; the struct's other fields follow, in
                // declaration order -- the same order `COLUMNS` lists and
                // `values` binds, all three being this one list.
                let mut index = 0;
                #(
                    let #columns = crate::db::Column::from_sql(row, { index += 1; index })?;
                )*
                Ok(#ty { id: Some(row.get(0)?), #(#columns,)* })
            }

            fn values(&self) -> ::std::vec::Vec<::rusqlite::types::Value> {
                ::std::vec![#(crate::db::Column::to_sql(&self.#columns)),*]
            }

            fn row_id(&self) -> ::std::option::Option<i64> {
                self.id
            }
        }
    })
}

/// Implements `crate::models::MarkdownBody` for a struct with a
/// `description: String` field.
///
/// The field is `#[serde(skip)]`ed on the struct itself and lives below the
/// YAML front matter as the markdown body instead, so every editable model
/// needs the same two accessors. They are the same two accessors every
/// time, which is what this derive is for.
///
/// ```ignore
/// #[derive(MarkdownBody)]
/// pub struct Task {
///     pub description: String,
/// }
/// ```
#[proc_macro_derive(MarkdownBody)]
pub fn derive_markdown_body(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let ty = &input.ident;
    let has_description = match &input.data {
        Data::Struct(data) => data.fields.iter().any(|field| {
            field
                .ident
                .as_ref()
                .is_some_and(|name| name == "description")
        }),
        _ => false,
    };
    if !has_description {
        return syn::Error::new(
            ty.span(),
            "`MarkdownBody` needs a struct with a `description: String` field",
        )
        .into_compile_error()
        .into();
    }
    quote! {
        impl crate::models::MarkdownBody for #ty {
            fn description(&self) -> &str {
                &self.description
            }

            fn set_description(&mut self, description: ::std::string::String) {
                self.description = description;
            }
        }
    }
    .into()
}

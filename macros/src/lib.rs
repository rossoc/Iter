//! The `Table` derive for `iter`. It lives in its own crate only because
//! Rust requires that of a procedural macro; nothing else depends on it.

use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, LitStr, parse_macro_input};

/// Implements `crate::db::Table` for a struct. Columns are the fields in declaration order, minus
/// the `id` primary key (assigned on insert; read from column 0). Other field types need
/// `FromSql`/`ToSql`, or `#[table(with = "path")]` naming a `crate::db` module with
/// `read(row, index)`/`write(&value)` (e.g. `datetime`). `order_by` sets `Db::list`'s trailing
/// clause. `kind = "project"` also implements `crate::models::Named` over `name`; its `validate`
/// requires a name and, with `check = "method"`, runs that `fn(&mut self) -> Result<()>`.
///
/// ```ignore
/// #[derive(Table)]
/// #[table(name = "projects", order_by = "name", kind = "project", check = "check")]
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

    let attr = input
        .attrs
        .iter()
        .find(|attr| attr.path().is_ident("table"))
        .ok_or_else(|| {
            syn::Error::new(ty.span(), r#"missing `#[table(name = "...")]` attribute"#)
        })?;
    let (mut name, mut order_by, mut kind, mut check) = (None, None, None, None);
    attr.parse_nested_meta(|meta| {
        let target = if meta.path.is_ident("name") {
            &mut name
        } else if meta.path.is_ident("order_by") {
            &mut order_by
        } else if meta.path.is_ident("kind") {
            &mut kind
        } else if meta.path.is_ident("check") {
            &mut check
        } else {
            return Err(meta.error("expected `name`, `order_by`, `kind` or `check`"));
        };
        *target = Some(meta.value()?.parse::<LitStr>()?);
        Ok(())
    })?;
    let name = name.ok_or_else(|| {
        syn::Error::new_spanned(attr, r#"`#[table(...)]` needs `name = "<sql table>"`"#)
    })?;
    let list_tail = order_by.map_or(String::new(), |by| format!("ORDER BY {}", by.value()));

    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new(ty.span(), "`Table` needs a struct"));
    };
    let Fields::Named(fields) = &data.fields else {
        return Err(syn::Error::new(
            ty.span(),
            "`Table` needs named fields: each one maps to a column",
        ));
    };
    let mut columns = Vec::new();
    let (mut reads, mut writes) = (Vec::new(), Vec::new());
    for field in fields
        .named
        .iter()
        .filter(|field| field.ident.as_ref().is_some_and(|name| name != "id"))
    {
        let name = field.ident.as_ref().expect("named field");
        let mut with = None;
        for attr in field.attrs.iter().filter(|a| a.path().is_ident("table")) {
            attr.parse_nested_meta(|meta| {
                if !meta.path.is_ident("with") {
                    return Err(meta.error("expected `with`"));
                }
                with = Some(meta.value()?.parse::<LitStr>()?.parse::<syn::Path>()?);
                Ok(())
            })?;
        }
        let ty = &field.ty;
        let index = columns.len() + 1;
        match with {
            Some(path) => {
                reads.push(quote! { crate::db::#path::read(row, #index)? });
                writes.push(quote! { crate::db::#path::write(&self.#name) });
            }
            None => {
                reads.push(quote! { row.get::<_, #ty>(#index)? });
                writes.push(quote! { ::rusqlite::types::ToSql::to_sql(&self.#name) });
            }
        }
        columns.push(name);
    }
    if columns.is_empty() {
        return Err(syn::Error::new(
            ty.span(),
            "`Table` needs at least one field besides `id` to use as a column",
        ));
    }
    let column_names = columns.iter().map(|field| field.to_string());

    let named = match (kind, check) {
        (None, None) => quote! {},
        (None, Some(check)) => {
            return Err(syn::Error::new_spanned(check, "`check` needs a `kind`"));
        }
        (Some(kind), check) => {
            let validate = check.map(|check| {
                let check = syn::Ident::new(&check.value(), check.span());
                quote! {
                    fn validate(&mut self) -> crate::error::Result<()> {
                        // `" work "` would be stored as typed and never found by `work`
                        self.name = self.name.trim().to_string();
                        crate::models::require_name(#kind, &self.name)?;
                        self.#check()
                    }
                }
            });
            quote! {
                impl crate::models::Named for #ty {
                    const KIND: &'static str = #kind;

                    fn name(&self) -> &str {
                        &self.name
                    }

                    #validate
                }
            }
        }
    };

    Ok(quote! {
        #named

        impl crate::db::Table for #ty {
            const NAME: &'static str = #name;
            const COLUMNS: &'static [&'static str] = &[#(#column_names),*];
            const LIST_TAIL: &'static str = #list_tail;

            fn from_row(row: &::rusqlite::Row) -> ::rusqlite::Result<Self> {
                // Column 0 is `id`; fields follow in declaration order (`COLUMNS` and `values`).
                Ok(#ty { id: Some(row.get(0)?), #(#columns: #reads,)* })
            }

            fn values(&self) -> ::rusqlite::Result<::std::vec::Vec<::rusqlite::types::ToSqlOutput<'_>>> {
                ::std::vec![#(#writes),*].into_iter().collect()
            }

            fn row_id(&self) -> ::std::option::Option<i64> {
                self.id
            }
        }
    })
}

/// Implements `crate::models::MarkdownBody` for a struct with a `description: String`, which is
/// `#[serde(skip)]`ed and lives below the YAML front matter. Also writes `carry_over`, copying the
/// buffer-less `#[serde(skip)]` fields (e.g. `project_id`) from the opened item.
#[proc_macro_derive(MarkdownBody)]
pub fn derive_markdown_body(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let ty = &input.ident;
    let fields: Vec<&syn::Field> = match &input.data {
        Data::Struct(data) => data.fields.iter().collect(),
        _ => Vec::new(),
    };
    let has_description = fields.iter().any(|field| {
        field
            .ident
            .as_ref()
            .is_some_and(|name| name == "description")
    });
    let hidden = fields
        .iter()
        .filter(|field| serde_skipped(field))
        .filter_map(|field| field.ident.as_ref())
        .filter(|name| *name != "description");
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

            fn carry_over(&mut self, original: &Self) {
                #( self.#hidden = ::std::clone::Clone::clone(&original.#hidden); )*
            }
        }
    }
    .into()
}

fn serde_skipped(field: &syn::Field) -> bool {
    field.attrs.iter().any(|attr| {
        let mut skip = false;
        if attr.path().is_ident("serde") {
            let _ = attr.parse_nested_meta(|meta| {
                skip |= meta.path.is_ident("skip");
                // Consume a `= value` or `(...)` so the other keys parse.
                if meta.input.peek(syn::Token![=]) {
                    meta.value()?.parse::<syn::Expr>()?;
                } else if meta.input.peek(syn::token::Paren) {
                    meta.parse_nested_meta(|_| Ok(()))?;
                }
                Ok(())
            });
        }
        skip
    })
}

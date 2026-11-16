/// Embedded default templates using compile-time inclusion
///
/// These templates are bundled into the binary and serve as fallbacks
/// when custom templates are not available.

/// Nota di seduta generale (modello predefinito)
pub const NOTA_DI_SEDUTA: &str = include_str!("../../../templates/nota_di_seduta.json");

/// Nota clinica in formato SOAP
pub const NOTA_SOAP: &str = include_str!("../../../templates/nota_soap.json");

/// Verifica dell'andamento del trattamento
pub const ANDAMENTO_TRATTAMENTO: &str =
    include_str!("../../../templates/andamento_trattamento.json");

/// Primo colloquio
pub const COLLOQUIO_INIZIALE: &str = include_str!("../../../templates/colloquio_iniziale.json");

/// Registry of all built-in templates
///
/// Maps template identifiers to their embedded JSON content
pub fn get_builtin_templates() -> Vec<(&'static str, &'static str)> {
    vec![
        ("nota_di_seduta", NOTA_DI_SEDUTA),
        ("nota_soap", NOTA_SOAP),
        ("andamento_trattamento", ANDAMENTO_TRATTAMENTO),
        ("colloquio_iniziale", COLLOQUIO_INIZIALE),
    ]
}

/// Get a built-in template by identifier
///
/// # Returns
/// The template JSON content if found, None otherwise
pub fn get_builtin_template(id: &str) -> Option<&'static str> {
    get_builtin_templates()
        .into_iter()
        .find(|(template_id, _)| *template_id == id)
        .map(|(_, content)| content)
}

/// List all built-in template identifiers
pub fn list_builtin_template_ids() -> Vec<&'static str> {
    get_builtin_templates().into_iter().map(|(id, _)| id).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_templates_valid_json() {
        for (id, content) in get_builtin_templates() {
            let result = serde_json::from_str::<serde_json::Value>(content);
            assert!(
                result.is_ok(),
                "Built-in template '{}' contains invalid JSON: {:?}",
                id,
                result.err()
            );
        }
    }

    #[test]
    fn test_get_builtin_template() {
        assert!(get_builtin_template("nota_di_seduta").is_some());
        assert!(get_builtin_template("nota_soap").is_some());
        assert!(get_builtin_template("nonexistent").is_none());
    }
}

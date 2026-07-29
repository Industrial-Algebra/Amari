// SPDX-License-Identifier: MIT OR Apache-2.0

pub mod wire {
    #[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
    #[serde(rename_all = "snake_case")]
    pub enum WireSchemaRole {
        Input,
        Output,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
    #[serde(rename_all = "snake_case")]
    pub enum WireCompatibility {
        AdditivePatch,
        VersionedChange,
    }

    #[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
    pub struct WireSemanticConstraint {
        pub id: String,
        pub description: String,
    }

    impl WireSemanticConstraint {
        pub fn new(
            id: impl Into<String>,
            description: impl Into<String>,
        ) -> Self {
            Self {
                id: id.into(),
                description: description.into(),
            }
        }
    }

    #[derive(Clone, Debug, PartialEq, serde::Serialize)]
    pub struct WireExample {
        pub label: String,
        pub value: serde_json::Value,
    }

    impl WireExample {
        pub fn new(label: impl Into<String>, value: serde_json::Value) -> Self {
            Self {
                label: label.into(),
                value,
            }
        }
    }

    pub trait WireContract {
        const SCHEMA_ID: &'static str;
        const ROLE: WireSchemaRole;
        const COMPATIBILITY: WireCompatibility;

        fn structural_schema() -> serde_json::Value;
        fn semantic_constraints() -> Vec<WireSemanticConstraint>;
        fn examples() -> Vec<WireExample>;
    }
}

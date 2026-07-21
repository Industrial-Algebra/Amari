// SPDX-License-Identifier: MIT OR Apache-2.0

#![cfg(feature = "ai")]

use amari_discovery::{
    AiContractLimits, AiExecutionRequest, Catalog, DiscoveryError, GoalInterpretation,
    GoalInterpretationRequest, GoalInterpreter, GoalSpec, ValidatedGoalInterpreter,
};

#[derive(Clone)]
struct FixedInterpreter {
    result: GoalInterpretation,
}

impl GoalInterpreter for FixedInterpreter {
    fn interpret(
        &self,
        _request: &GoalInterpretationRequest,
    ) -> amari_discovery::DiscoveryResult<GoalInterpretation> {
        Ok(self.result.clone())
    }
}

fn known_interpretation() -> GoalInterpretation {
    GoalInterpretation {
        goal: GoalSpec {
            statement: "decode a bounded state sequence".to_owned(),
            constraints: vec!["deterministic CPU execution".to_owned()],
        },
        capability_ids: vec!["amari:amari-tropical:sequence:viterbi".parse().unwrap()],
        missing_information: vec!["state count".to_owned()],
        execution_requests: Vec::new(),
    }
}

fn request() -> GoalInterpretationRequest {
    GoalInterpretationRequest {
        text: "I need to decode the most likely state sequence".to_owned(),
    }
}

#[test]
fn deterministic_in_process_adapter_returns_catalog_validated_goal() {
    let catalog = Catalog::embedded().unwrap();
    let adapter = FixedInterpreter {
        result: known_interpretation(),
    };
    let validator =
        ValidatedGoalInterpreter::new(&catalog, &adapter, AiContractLimits::default()).unwrap();

    let first = validator.interpret(&request()).unwrap();
    let second = validator.interpret(&request()).unwrap();
    assert_eq!(first, known_interpretation());
    assert_eq!(
        serde_json::to_vec(&first).unwrap(),
        serde_json::to_vec(&second).unwrap()
    );
}

#[test]
fn uncatalogued_capability_ids_are_rejected() {
    let catalog = Catalog::embedded().unwrap();
    let mut result = known_interpretation();
    result.capability_ids = vec!["amari:missing:domain:operation".parse().unwrap()];
    let adapter = FixedInterpreter { result };
    let validator =
        ValidatedGoalInterpreter::new(&catalog, &adapter, AiContractLimits::default()).unwrap();

    let error = validator.interpret(&request()).unwrap_err();
    assert!(matches!(error, DiscoveryError::InvalidId { .. }));
}

#[test]
fn any_execution_authority_requested_by_an_adapter_is_rejected() {
    let catalog = Catalog::embedded().unwrap();
    for execution_request in [
        AiExecutionRequest::RunProbe,
        AiExecutionRequest::ModifyProject,
        AiExecutionRequest::InvokeCommand,
        AiExecutionRequest::AccessNetwork,
    ] {
        let mut result = known_interpretation();
        result.execution_requests = vec![execution_request];
        let adapter = FixedInterpreter { result };
        let validator =
            ValidatedGoalInterpreter::new(&catalog, &adapter, AiContractLimits::default()).unwrap();

        let error = validator.interpret(&request()).unwrap_err();
        assert!(matches!(error, DiscoveryError::InvalidInput(_)));
        assert!(error.to_string().contains("execution authority"));
    }
}

#[test]
fn request_goal_references_questions_and_encoded_output_are_bounded() {
    let catalog = Catalog::embedded().unwrap();
    let limits = AiContractLimits {
        max_request_bytes: 8,
        max_capability_ids: 1,
        max_missing_information: 1,
        max_output_bytes: 512,
    };
    let adapter = FixedInterpreter {
        result: known_interpretation(),
    };
    let validator = ValidatedGoalInterpreter::new(&catalog, &adapter, limits).unwrap();
    assert!(matches!(
        validator.interpret(&request()).unwrap_err(),
        DiscoveryError::LimitExceeded(_)
    ));

    let mut too_many_capabilities = known_interpretation();
    too_many_capabilities
        .capability_ids
        .push("amari:amari-tropical:paths:shortest-path".parse().unwrap());
    let adapter = FixedInterpreter {
        result: too_many_capabilities,
    };
    let validator = ValidatedGoalInterpreter::new(
        &catalog,
        &adapter,
        AiContractLimits {
            max_request_bytes: 1024,
            ..limits
        },
    )
    .unwrap();
    assert!(matches!(
        validator.interpret(&request()).unwrap_err(),
        DiscoveryError::LimitExceeded(_)
    ));

    let mut too_many_questions = known_interpretation();
    too_many_questions
        .missing_information
        .push("observation count".to_owned());
    let adapter = FixedInterpreter {
        result: too_many_questions,
    };
    let validator = ValidatedGoalInterpreter::new(
        &catalog,
        &adapter,
        AiContractLimits {
            max_request_bytes: 1024,
            ..limits
        },
    )
    .unwrap();
    assert!(matches!(
        validator.interpret(&request()).unwrap_err(),
        DiscoveryError::LimitExceeded(_)
    ));

    let mut invalid_goal = known_interpretation();
    invalid_goal.goal.constraints = vec!["constraint".to_owned(); GoalSpec::MAX_CONSTRAINTS + 1];
    let adapter = FixedInterpreter {
        result: invalid_goal,
    };
    let validator = ValidatedGoalInterpreter::new(
        &catalog,
        &adapter,
        AiContractLimits {
            max_request_bytes: 1024,
            max_output_bytes: 1024 * 1024,
            ..limits
        },
    )
    .unwrap();
    assert!(matches!(
        validator.interpret(&request()).unwrap_err(),
        DiscoveryError::LimitExceeded(_)
    ));

    let mut oversized = known_interpretation();
    oversized.goal.statement = "x".repeat(1024);
    let adapter = FixedInterpreter { result: oversized };
    let validator = ValidatedGoalInterpreter::new(
        &catalog,
        &adapter,
        AiContractLimits {
            max_request_bytes: 1024,
            ..limits
        },
    )
    .unwrap();
    assert!(matches!(
        validator.interpret(&request()).unwrap_err(),
        DiscoveryError::LimitExceeded(_)
    ));
}

#[test]
fn zero_contract_limits_are_rejected_before_adapter_use() {
    let catalog = Catalog::embedded().unwrap();
    let adapter = FixedInterpreter {
        result: known_interpretation(),
    };
    let error = ValidatedGoalInterpreter::new(
        &catalog,
        &adapter,
        AiContractLimits {
            max_request_bytes: 0,
            ..AiContractLimits::default()
        },
    )
    .err()
    .unwrap();
    assert!(matches!(error, DiscoveryError::InvalidInput(_)));
}

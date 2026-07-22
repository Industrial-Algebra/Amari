// SPDX-License-Identifier: MIT OR Apache-2.0

//! Provider-neutral, in-process AI goal interpretation contract.
//!
//! This module defines validation boundaries only. It ships no provider,
//! subprocess transport, network client, probe authority, or project mutation.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{CapabilityId, Catalog, DiscoveryError, DiscoveryResult, GoalSpec};

/// Resource ceilings applied around one goal-interpreter invocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiContractLimits {
    /// Maximum UTF-8 bytes accepted in one natural-language request.
    pub max_request_bytes: usize,
    /// Maximum catalog capability references accepted from an adapter.
    pub max_capability_ids: usize,
    /// Maximum missing-information questions accepted from an adapter.
    pub max_missing_information: usize,
    /// Maximum encoded bytes accepted in one adapter result.
    pub max_output_bytes: usize,
}

impl Default for AiContractLimits {
    fn default() -> Self {
        Self {
            max_request_bytes: 16 * 1024,
            max_capability_ids: 64,
            max_missing_information: 32,
            max_output_bytes: 64 * 1024,
        }
    }
}

impl AiContractLimits {
    fn validate(self) -> DiscoveryResult<()> {
        if self.max_request_bytes == 0
            || self.max_capability_ids == 0
            || self.max_missing_information == 0
            || self.max_output_bytes == 0
        {
            return Err(DiscoveryError::InvalidInput(
                "AI contract limits must be positive".to_owned(),
            ));
        }
        Ok(())
    }
}

/// Bounded natural-language input supplied to a [`GoalInterpreter`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoalInterpretationRequest {
    /// Natural-language statement to translate into a typed goal.
    pub text: String,
}

/// Execution authority an untrusted adapter attempted to request.
///
/// Every variant is forbidden by [`ValidatedGoalInterpreter`]. The variants
/// make attempted authority explicit instead of accepting opaque commands.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiExecutionRequest {
    /// Attempt to execute a registered probe directly.
    RunProbe,
    /// Attempt to modify the inspected project.
    ModifyProject,
    /// Attempt to invoke a command or external process.
    InvokeCommand,
    /// Attempt to access a provider or other network endpoint.
    AccessNetwork,
}

/// Typed output proposed by a provider-neutral goal interpreter.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoalInterpretation {
    /// Deterministic planner goal proposed by the adapter.
    pub goal: GoalSpec,
    /// Catalog capabilities referenced by the interpretation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capability_ids: Vec<CapabilityId>,
    /// Missing information that a human shell may ask for explicitly.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing_information: Vec<String>,
    /// Forbidden authority requests disclosed by the adapter.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub execution_requests: Vec<AiExecutionRequest>,
}

/// Provider-neutral in-process goal interpretation interface.
///
/// Implementations receive typed bounded input and return a proposed typed
/// interpretation. Callers should invoke implementations only through
/// [`ValidatedGoalInterpreter`].
pub trait GoalInterpreter {
    /// Interprets natural-language input into a proposed planner goal.
    ///
    /// # Errors
    ///
    /// Returns a structured error when interpretation cannot be completed.
    fn interpret(&self, request: &GoalInterpretationRequest)
        -> DiscoveryResult<GoalInterpretation>;
}

/// Catalog and authority validation around an in-process [`GoalInterpreter`].
pub struct ValidatedGoalInterpreter<'a, I: ?Sized> {
    catalog: &'a Catalog,
    interpreter: &'a I,
    limits: AiContractLimits,
}

impl<'a, I: GoalInterpreter + ?Sized> ValidatedGoalInterpreter<'a, I> {
    /// Creates a validation wrapper with explicit positive resource ceilings.
    ///
    /// # Errors
    ///
    /// Returns [`DiscoveryError::InvalidInput`] when any ceiling is zero.
    pub fn new(
        catalog: &'a Catalog,
        interpreter: &'a I,
        limits: AiContractLimits,
    ) -> DiscoveryResult<Self> {
        limits.validate()?;
        Ok(Self {
            catalog,
            interpreter,
            limits,
        })
    }

    /// Interprets a request and validates goal, catalog, size, and authority.
    ///
    /// # Errors
    ///
    /// Returns a structured error for empty or oversized requests, invalid
    /// goals, unknown or duplicate catalog references, oversized output, empty
    /// missing-information entries, or any requested execution authority.
    pub fn interpret(
        &self,
        request: &GoalInterpretationRequest,
    ) -> DiscoveryResult<GoalInterpretation> {
        let request_bytes = request.text.len();
        if request.text.trim().is_empty() {
            return Err(DiscoveryError::InvalidInput(
                "AI goal interpretation request must not be empty".to_owned(),
            ));
        }
        if request_bytes > self.limits.max_request_bytes {
            return Err(DiscoveryError::LimitExceeded(format!(
                "AI request bytes {request_bytes} exceed limit {}",
                self.limits.max_request_bytes
            )));
        }

        let interpretation = self.interpreter.interpret(request)?;
        interpretation.goal.validate()?;
        if interpretation.capability_ids.len() > self.limits.max_capability_ids {
            return Err(DiscoveryError::LimitExceeded(format!(
                "AI capability references {} exceed limit {}",
                interpretation.capability_ids.len(),
                self.limits.max_capability_ids
            )));
        }
        if interpretation.missing_information.len() > self.limits.max_missing_information {
            return Err(DiscoveryError::LimitExceeded(format!(
                "AI missing-information entries {} exceed limit {}",
                interpretation.missing_information.len(),
                self.limits.max_missing_information
            )));
        }
        if interpretation
            .missing_information
            .iter()
            .any(|item| item.trim().is_empty())
        {
            return Err(DiscoveryError::InvalidInput(
                "AI missing-information entries must not be empty".to_owned(),
            ));
        }
        if !interpretation.execution_requests.is_empty() {
            return Err(DiscoveryError::InvalidInput(
                "AI adapter requested prohibited execution authority".to_owned(),
            ));
        }

        let mut seen = BTreeSet::new();
        for capability_id in &interpretation.capability_ids {
            if !seen.insert(capability_id) {
                return Err(DiscoveryError::InvalidInput(format!(
                    "AI adapter returned duplicate capability `{capability_id}`"
                )));
            }
            if !self
                .catalog
                .capabilities()
                .iter()
                .any(|capability| &capability.id == capability_id)
            {
                return Err(DiscoveryError::invalid_id(
                    capability_id.to_string(),
                    "AI adapter referenced a capability outside the embedded catalog",
                ));
            }
        }

        let output_bytes = serde_json::to_vec(&interpretation)?.len();
        if output_bytes > self.limits.max_output_bytes {
            return Err(DiscoveryError::LimitExceeded(format!(
                "AI output bytes {output_bytes} exceed limit {}",
                self.limits.max_output_bytes
            )));
        }
        Ok(interpretation)
    }
}

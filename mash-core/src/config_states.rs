//! TypeState helpers for installer configs (validation -> arming -> executing).
//!
//! This encodes safety invariants in types so destructive operations can only be invoked once a
//! config is validated and explicitly armed.

use crate::errors::MashError;
use anyhow::Result;

pub const SAFE_MODE_DISARM_CONFIRMATION: &str = "DESTROY";
pub const EXECUTE_CONFIRMATION: &str = "I UNDERSTAND THIS WILL ERASE THE SELECTED DISK";

#[derive(Debug, Clone, Copy)]
pub struct ExecuteArmToken(());

impl ExecuteArmToken {
    pub fn try_new(
        yes_i_know: bool,
        safe_mode_disarmed: bool,
        execute_confirmation_ok: bool,
    ) -> Result<Self> {
        if !yes_i_know {
            return Err(MashError::MissingYesIKnow.into());
        }
        if !safe_mode_disarmed {
            return Err(MashError::MissingSafeModeDisarm.into());
        }
        if !execute_confirmation_ok {
            return Err(MashError::MissingExecuteConfirmation.into());
        }
        Ok(Self(()))
    }
}

pub trait ValidateConfig {
    fn validate_cfg(&self) -> Result<()>;
}

pub trait HasRunMode {
    fn is_dry_run(&self) -> bool;
}

pub struct Unvalidated;
pub struct Validated;
pub struct Armed;
pub struct Executing;

#[derive(Debug, Clone)]
pub struct UnvalidatedConfig<T>(pub T);

#[derive(Debug, Clone)]
pub struct ValidatedConfig<T>(pub T);

#[derive(Debug, Clone)]
pub struct ArmedConfig<T> {
    pub cfg: T,
    pub token: ExecuteArmToken,
}

#[derive(Debug, Clone)]
pub struct ExecutingConfig<T> {
    pub cfg: T,
    pub token: ExecuteArmToken,
}

impl<T> UnvalidatedConfig<T> {
    pub fn new(cfg: T) -> Self {
        Self(cfg)
    }
}

impl<T: ValidateConfig> UnvalidatedConfig<T> {
    pub fn validate(self) -> Result<ValidatedConfig<T>> {
        self.0.validate_cfg()?;
        Ok(ValidatedConfig(self.0))
    }
}

impl<T: HasRunMode> ValidatedConfig<T> {
    pub fn require_dry_run(&self) -> Result<()> {
        if !self.0.is_dry_run() {
            anyhow::bail!("expected dry-run config");
        }
        Ok(())
    }

    pub fn arm_execute(self, token: ExecuteArmToken) -> Result<ArmedConfig<T>> {
        if self.0.is_dry_run() {
            anyhow::bail!("cannot arm an execute token for a dry-run config");
        }
        Ok(ArmedConfig { cfg: self.0, token })
    }
}

impl<T> ArmedConfig<T> {
    pub fn into_executing(self) -> ExecutingConfig<T> {
        ExecutingConfig {
            cfg: self.cfg,
            token: self.token,
        }
    }
}

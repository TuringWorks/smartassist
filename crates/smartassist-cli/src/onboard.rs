//! Legacy onboarding wizard.
//!
//! Deprecated in favor of [`crate::wizard::SetupWizard`] which produces
//! validated configuration using the `ConfigBuilder`.
//!
//! This module is retained for backward compatibility. The `OnboardWizard`
//! now delegates to the new wizard.

/// The legacy onboarding wizard. Delegates to [`crate::wizard::SetupWizard`].
pub struct OnboardWizard {
    force: bool,
}

impl OnboardWizard {
    /// Create a new wizard.
    pub fn new(force: bool) -> Self {
        Self { force }
    }

    /// Run the wizard (delegates to the enhanced wizard).
    pub async fn run(&self) -> anyhow::Result<()> {
        crate::wizard::SetupWizard::new(self.force, false)
            .run()
            .await
    }
}

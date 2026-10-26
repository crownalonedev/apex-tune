use tauri::State;

use super::safety::{EngineState, blocking};
use crate::error::{AppError, AppResult};
use crate::power::{self, PowerOverview};
use crate::tweaks::TweakError;

#[tauri::command]
pub async fn power_overview(state: State<'_, EngineState>) -> AppResult<PowerOverview> {
    blocking(state, |e| {
        Ok(power::overview(e.ctx()).map_err(TweakError::from)?)
    })
    .await
}

/// Makes another installed plan active. Journaled, so it can be reverted.
#[tauri::command]
pub async fn power_switch(state: State<'_, EngineState>, guid: String) -> AppResult<PowerOverview> {
    blocking(state, move |e| {
        let plans = e.ctx().power.schemes().map_err(TweakError::from)?;
        let plan = plans
            .iter()
            .find(|p| p.guid.eq_ignore_ascii_case(&guid))
            .ok_or_else(|| AppError::Other("That power plan isn't installed.".into()))?;
        let name = format!("Switched power plan to {}", plan.name);
        e.apply_action("power.switch", &name, power::switch_atoms(&guid))?;
        Ok(power::overview(e.ctx()).map_err(TweakError::from)?)
    })
    .await
}

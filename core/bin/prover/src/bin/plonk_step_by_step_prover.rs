use zksync_prover::cli_utils::main_for_prover_impl;
use zksync_prover::plonk_step_by_step_prover::PlonkStepByStepProver;

#[tokio::main]
async fn main() {
    let run_prometheus_exporter = false;
    main_for_prover_impl::<PlonkStepByStepProver>(run_prometheus_exporter).await;
}

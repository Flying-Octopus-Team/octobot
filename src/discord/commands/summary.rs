use tracing::info;

use crate::database::models::report::Report;

pub(crate) async fn generate_summary() -> Result<String, Box<dyn std::error::Error>> {
    info!("Generating summary");

    let output = Report::report_summary(None).await?;

    if output.is_empty() {
        info!("No reports to summarize");
        Ok("No unpublished reports".to_string())
    } else {
        info!("Summary generated");
        Ok(output)
    }
}

use daml_lint::detector::{ConfiguredDetector, DetectError, Detector, Finding, Severity};
use daml_lint::ir::DamlModule;

struct DummyDetector;

impl Detector for DummyDetector {
    fn name(&self) -> &str {
        "dummy"
    }

    fn severity(&self) -> Severity {
        Severity::Low
    }

    fn description(&self) -> &str {
        "dummy detector"
    }

    fn try_detect(&self, _: &DamlModule) -> Result<Vec<Finding>, DetectError> {
        Ok(Vec::new())
    }
}

fn main() {
    let _detector = ConfiguredDetector::new(Box::new(DummyDetector), None, None);
}

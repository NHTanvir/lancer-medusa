//! HTML coverage report generation.

#![allow(dead_code)]

use std::collections::HashMap;

use alloy_primitives::Address;

use crate::fuzzing::coverage::ContractCoverageMap;
use crate::fuzzing::test_case::{TestCaseResult, TestStatus};

// ── CoverageReport ────────────────────────────────────────────────────────────

/// Aggregated results for a completed (or interrupted) fuzz campaign.
pub struct CoverageReport {
    /// Per-contract coverage data.
    pub per_contract: HashMap<Address, ContractCoverageMap>,
    /// All test case results collected during the campaign.
    pub test_results: Vec<TestCaseResult>,
    /// Total number of call sequences executed.
    pub total_sequences: usize,
}

impl CoverageReport {
    /// Create a new `CoverageReport`.
    pub fn new(
        per_contract: HashMap<Address, ContractCoverageMap>,
        test_results: Vec<TestCaseResult>,
        total_sequences: usize,
    ) -> Self {
        Self {
            per_contract,
            test_results,
            total_sequences,
        }
    }

    /// Render the report as a self-contained HTML string.
    pub fn render_html(&self) -> String {
        let mut html = String::new();

        // ── head ──────────────────────────────────────────────────────────────
        html.push_str("<!DOCTYPE html>\n<html>\n<head>\n");
        html.push_str("<meta charset=\"utf-8\">\n");
        html.push_str("<title>lancer-medusa Coverage Report</title>\n");
        html.push_str("<style>");
        html.push_str("body{font-family:monospace;background:#1a1a1a;color:#e0e0e0;padding:2em}");
        html.push_str("h1{color:#00ff88}h2{color:#00aaff}h3{color:#cccccc}");
        html.push_str("ul{list-style:none;padding:0}li{padding:0.2em 0}");
        html.push_str(".fail{color:#ff4444}.pass{color:#44ff44}");
        html.push_str(".contract{border:1px solid #333;padding:1em;margin:1em 0;border-radius:4px}");
        html.push_str("</style>\n");
        html.push_str("</head>\n<body>\n");

        // ── title ─────────────────────────────────────────────────────────────
        html.push_str("<h1>lancer-medusa Coverage Report</h1>\n");

        // ── summary ───────────────────────────────────────────────────────────
        let total_markers: usize = self
            .per_contract
            .values()
            .map(|m| m.marker_count())
            .sum();
        let failed = self
            .test_results
            .iter()
            .filter(|r| r.status == TestStatus::Failed)
            .count();
        let passed = self
            .test_results
            .iter()
            .filter(|r| r.status == TestStatus::Passed)
            .count();

        html.push_str("<h2>Summary</h2>\n<ul>\n");
        html.push_str(&format!(
            "<li>Total sequences: {}</li>\n",
            self.total_sequences
        ));
        html.push_str(&format!(
            "<li>Total coverage markers: {}</li>\n",
            total_markers
        ));
        html.push_str(&format!(
            "<li class=\"fail\">Failed tests: {}</li>\n",
            failed
        ));
        html.push_str(&format!(
            "<li class=\"pass\">Passed/Optimization tests: {}</li>\n",
            passed
        ));
        html.push_str("</ul>\n");

        // ── per-contract coverage ─────────────────────────────────────────────
        html.push_str("<h2>Coverage by Contract</h2>\n");
        if self.per_contract.is_empty() {
            html.push_str("<p>No contract coverage data recorded.</p>\n");
        } else {
            for (addr, cmap) in &self.per_contract {
                html.push_str("<div class=\"contract\">\n");
                html.push_str(&format!("<h3>Contract: {:?}</h3>\n", addr));
                html.push_str(&format!(
                    "<p>Coverage markers: {}</p>\n",
                    cmap.marker_count()
                ));
                html.push_str("</div>\n");
            }
        }

        // ── test results ──────────────────────────────────────────────────────
        if !self.test_results.is_empty() {
            html.push_str("<h2>Test Results</h2>\n");
            for result in &self.test_results {
                let cls = match result.status {
                    TestStatus::Failed => "fail",
                    TestStatus::Passed => "pass",
                    TestStatus::Unresolved => "",
                };
                html.push_str(&format!("<div class=\"contract {}\">\n", cls));
                html.push_str(&format!(
                    "<b>{}</b> ({}): {}\n",
                    result.name, result.contract, result.message
                ));
                html.push_str("</div>\n");
            }
        }

        html.push_str("</body>\n</html>\n");
        html
    }

    /// Write the HTML report to `path`.
    pub fn write_html(&self, path: &std::path::Path) -> anyhow::Result<()> {
        let html = self.render_html();
        std::fs::write(path, html)?;
        Ok(())
    }
}

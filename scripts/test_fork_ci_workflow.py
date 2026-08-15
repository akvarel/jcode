#!/usr/bin/env python3
"""Regression tests for CI behavior in forks without optional repository features."""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CI_WORKFLOW = ROOT / ".github" / "workflows" / "ci.yml"
ISSUE_WORKFLOW = ROOT / ".github" / "workflows" / "require-issue.yml"


class ForkCiWorkflowTests(unittest.TestCase):
    def test_optional_deploy_key_gates_every_ssh_agent_step(self) -> None:
        workflow = CI_WORKFLOW.read_text(encoding="utf-8")
        steps = re.findall(
            r"(?ms)^\s+- name: Configure SSH for cargo git dependencies\n"
            r"(?P<body>(?:^\s{8,}.*\n){1,8})",
            workflow,
        )
        self.assertGreater(len(steps), 0)
        self.assertEqual(workflow.count("DEPLOY_KEY: ${{ secrets.DEPLOY_KEY }}"), 4)
        for body in steps:
            self.assertIn("if: ${{ env.DEPLOY_KEY != '' }}", body)
            self.assertIn("uses: webfactory/ssh-agent@", body)
            self.assertIn("ssh-private-key: ${{ env.DEPLOY_KEY }}", body)

    def test_issue_policy_skips_when_repository_issues_are_disabled(self) -> None:
        workflow = ISSUE_WORKFLOW.read_text(encoding="utf-8")
        self.assertIn("hasIssuesEnabled", workflow)
        self.assertIn("Repository issues are disabled; skipping linked-issue requirement.", workflow)
        self.assertLess(
            workflow.index("hasIssuesEnabled"),
            workflow.index("const candidates = new Set()"),
        )


if __name__ == "__main__":
    unittest.main()

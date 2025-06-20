"""Rule manager for iterative auto-learning."""

import os
import json
from typing import List, Dict, Any
from pathlib import Path
from ..utils.config import get_config
from ..utils.logging import LoggerMixin
from .similarity import SimilarityChecker

class RuleManager(LoggerMixin):
    """Manages learning rules and stores successful examples."""
    def __init__(self):
        self.config = get_config()
        self.examples_dir = Path(self.config.data.examples_dir)
        self.examples_dir.mkdir(parents=True, exist_ok=True)
        self.similarity_checker = SimilarityChecker()
        self.max_examples = self.config.learning.max_examples_per_rule
        self.examples: Dict[str, List[Dict[str, Any]]] = {}
        self._load_examples()

    def _get_rule_file(self, rule_name: str) -> Path:
        return self.examples_dir / f"{rule_name}_examples.json"

    def _load_examples(self):
        for rule in self.config.learning.rules:
            rule_name = rule["name"]
            rule_file = self._get_rule_file(rule_name)
            if rule_file.exists():
                with open(rule_file, "r", encoding="utf-8") as f:
                    self.examples[rule_name] = json.load(f)
            else:
                self.examples[rule_name] = []

    def _save_examples(self, rule_name: str):
        rule_file = self._get_rule_file(rule_name)
        with open(rule_file, "w", encoding="utf-8") as f:
            json.dump(self.examples[rule_name], f, indent=2, ensure_ascii=False)

    def evaluate_and_learn(self, input_text: str, dsl_commands: str, output_description: str):
        """
        If input and output are similar, add as an example for the best-matching rule.
        """
        for rule in self.config.learning.rules:
            rule_name = rule["name"]
            # For now, use similarity only; can extend with rule-specific logic
            if self.similarity_checker.is_similar(input_text, output_description):
                self.logger.info(f"Adding example to rule '{rule_name}'")
                self._add_example(rule_name, input_text, dsl_commands, output_description)
                break

    def _add_example(self, rule_name: str, input_text: str, dsl_commands: str, output_description: str):
        example = {
            "input": input_text,
            "dsl": dsl_commands,
            "description": output_description
        }
        examples = self.examples[rule_name]
        examples.append(example)
        # Keep only the most recent max_examples
        if len(examples) > self.max_examples:
            examples = examples[-self.max_examples:]
        self.examples[rule_name] = examples
        self._save_examples(rule_name)

    def get_examples(self, rule_name: str) -> List[Dict[str, Any]]:
        return self.examples.get(rule_name, [])

    def all_examples(self) -> Dict[str, List[Dict[str, Any]]]:
        return self.examples

    def clear_examples(self, rule_name: str):
        self.examples[rule_name] = []
        self._save_examples(rule_name) 
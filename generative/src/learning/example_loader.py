"""Example loader for bootstrapping learning with example prompts."""

import json
from pathlib import Path
from typing import List, Dict, Any, Optional
from ..utils.config import get_config
from ..utils.logging import LoggerMixin


class ExampleLoader(LoggerMixin):
    """Loads and manages example prompts for bootstrapping learning."""
    
    def __init__(self):
        self.config = get_config()
        self.examples_dir = Path(self.config.data.examples_dir)
        self.examples_dir.mkdir(parents=True, exist_ok=True)
        self.examples = []
        self._load_examples()
    
    def _load_examples(self):
        """Load example prompts from JSON file."""
        example_file = self.examples_dir / "example_prompts.json"
        try:
            if example_file.exists():
                with open(example_file, 'r', encoding='utf-8') as f:
                    self.examples = json.load(f)
                self.logger.info(f"Loaded {len(self.examples)} example prompts")
            else:
                self.logger.warning(f"Example prompts file not found: {example_file}")
        except Exception as e:
            self.logger.error(f"Failed to load example prompts: {e}")
    
    def get_examples(self) -> List[Dict[str, Any]]:
        """Get all loaded examples."""
        return self.examples
    
    def add_example(self, input_text: str, dsl_commands: str, description: str):
        """Add a new example to the examples list."""
        example = {
            "input": input_text,
            "dsl": dsl_commands,
            "description": description
        }
        self.examples.append(example)
        self._save_examples()
        self.logger.info(f"Added new example: {input_text[:50]}...")
    
    def _save_examples(self):
        """Save examples to JSON file."""
        example_file = self.examples_dir / "example_prompts.json"
        try:
            with open(example_file, 'w', encoding='utf-8') as f:
                json.dump(self.examples, f, indent=2, ensure_ascii=False)
        except Exception as e:
            self.logger.error(f"Failed to save examples: {e}")
    
    def get_similar_examples(self, input_text: str, top_k: int = 3) -> List[Dict[str, Any]]:
        """
        Get examples similar to the input text.
        
        Args:
            input_text: Input text to find similar examples for
            top_k: Number of similar examples to return
            
        Returns:
            List of similar examples
        """
        # Simple keyword-based similarity for now
        input_lower = input_text.lower()
        scored_examples = []
        
        for example in self.examples:
            score = 0
            example_text = example["input"].lower()
            
            # Count common words
            input_words = set(input_lower.split())
            example_words = set(example_text.split())
            common_words = input_words.intersection(example_words)
            score += len(common_words)
            
            # Bonus for exact matches
            if input_lower == example_text:
                score += 10
            
            scored_examples.append((score, example))
        
        # Sort by score and return top_k
        scored_examples.sort(key=lambda x: x[0], reverse=True)
        return [example for score, example in scored_examples[:top_k]]
    
    def format_examples_for_prompt(self, input_text: str, max_examples: int = 3) -> str:
        """
        Format examples for inclusion in a prompt.
        
        Args:
            input_text: Input text to find relevant examples for
            max_examples: Maximum number of examples to include
            
        Returns:
            Formatted string with examples
        """
        similar_examples = self.get_similar_examples(input_text, max_examples)
        
        if not similar_examples:
            return ""
        
        examples_text = "\n\nExamples:\n"
        for i, example in enumerate(similar_examples, 1):
            examples_text += f"Example {i}:\n"
            examples_text += f"Input: \"{example['input']}\"\n"
            examples_text += f"Output:\n{example['dsl']}\n"
        
        return examples_text 
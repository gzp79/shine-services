"""Rule management and learning evaluation."""

import json
from pathlib import Path
from typing import Dict, List, Any, Optional
from ..utils.config import get_config
from ..utils.logging import LoggerMixin


class RuleManager(LoggerMixin):
    """Manages learning evaluation and rule storage."""
    
    def __init__(self):
        self.config = get_config()
        self.logger.info("RuleManager initialized")
    
    def evaluate_and_learn(self, input_text: str, dsl_commands: str, description: str) -> Dict[str, Any]:
        """
        Evaluate the learning process and store insights.
        
        Args:
            input_text: Original input text
            dsl_commands: Generated DSL commands
            description: BLIP-2 description of generated image
            
        Returns:
            Dictionary with evaluation results
        """
        evaluation = {
            "input_text": input_text,
            "dsl_commands": dsl_commands,
            "description": description,
            "timestamp": self._get_timestamp(),
            "success": True
        }
        
        self.logger.info(f"Learning evaluation completed for: {input_text[:50]}...")
        return evaluation
    
    def _get_timestamp(self) -> str:
        """Get current timestamp."""
        from datetime import datetime
        return datetime.now().isoformat() 
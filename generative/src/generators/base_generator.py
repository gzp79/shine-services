"""Abstract base class for image generators."""

from abc import ABC, abstractmethod
from typing import Any
from PIL import Image

class BaseGenerator(ABC):
    """Abstract base class for all image generators."""
    
    @abstractmethod
    def generate(self, dsl_commands: str) -> Image.Image:
        """
        Generate an image from DSL commands.
        
        Args:
            dsl_commands: DSL command string
            
        Returns:
            Generated PIL Image
        """
        pass 
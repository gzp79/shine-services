"""Exploratory learning module for discovering new patterns through random DSL generation."""

import random
import json
import os
import datetime
from pathlib import Path
from typing import List, Dict, Any, Optional, Tuple
from PIL import Image

from ..utils.config import get_config
from ..utils.logging import LoggerMixin
from ..generators.placeholder_generator import PlaceholderGenerator
from ..models.blip_descriptor import BLIPDescriptor
from ..learning.example_loader import ExampleLoader


class ExploratoryLearner(LoggerMixin):
    """Generates random DSL commands and learns from confident BLIP descriptions."""
    
    def __init__(self):
        self.config = get_config()
        self.generator = PlaceholderGenerator()
        self.descriptor = BLIPDescriptor()
        self.example_loader = ExampleLoader()
        
        # DSL command templates for random generation
        self.dsl_templates = self._load_dsl_templates()
        self.colors = ["red", "green", "blue", "yellow", "orange", "purple", "brown", "black", "white", "gray", "pink", "cyan", "magenta", "lime", "navy", "teal", "maroon", "olive"]
        
        self.logger.info("ExploratoryLearner initialized")
    
    def _load_dsl_templates(self) -> List[Dict[str, Any]]:
        """Load DSL command templates for complex random generation."""
        return [
            # Basic shapes with more variety
            {
                "command": "fill_circle",
                "params": {
                    "x": (20, 492),
                    "y": (20, 492),
                    "radius": (5, 150),
                    "color": "colors"
                },
                "weight": 0.15
            },
            {
                "command": "draw_circle",
                "params": {
                    "x": (20, 492),
                    "y": (20, 492),
                    "radius": (5, 150),
                    "color": "colors"
                },
                "weight": 0.12
            },
            {
                "command": "fill_rect",
                "params": {
                    "x": (20, 450),
                    "y": (20, 450),
                    "w": (10, 250),
                    "h": (10, 250),
                    "color": "colors"
                },
                "weight": 0.15
            },
            {
                "command": "draw_rect",
                "params": {
                    "x": (20, 450),
                    "y": (20, 450),
                    "w": (10, 250),
                    "h": (10, 250),
                    "color": "colors"
                },
                "weight": 0.12
            },
            {
                "command": "fill_ellipse",
                "params": {
                    "x": (20, 450),
                    "y": (20, 450),
                    "w": (10, 200),
                    "h": (10, 200),
                    "color": "colors"
                },
                "weight": 0.08
            },
            {
                "command": "draw_ellipse",
                "params": {
                    "x": (20, 450),
                    "y": (20, 450),
                    "w": (10, 200),
                    "h": (10, 200),
                    "color": "colors"
                },
                "weight": 0.08
            },
            # Lines with more variety
            {
                "command": "draw_line",
                "params": {
                    "x1": (20, 492),
                    "y1": (20, 492),
                    "x2": (20, 492),
                    "y2": (20, 492),
                    "color": "colors",
                    "width": (1, 15)
                },
                "weight": 0.12
            },
            # Triangles (if supported)
            {
                "command": "fill_triangle",
                "params": {
                    "x1": (20, 492),
                    "y1": (20, 492),
                    "x2": (20, 492),
                    "y2": (20, 492),
                    "x3": (20, 492),
                    "y3": (20, 492),
                    "color": "colors"
                },
                "weight": 0.05
            },
            {
                "command": "draw_triangle",
                "params": {
                    "x1": (20, 492),
                    "y1": (20, 492),
                    "x2": (20, 492),
                    "y2": (20, 492),
                    "x3": (20, 492),
                    "y3": (20, 492),
                    "color": "colors"
                },
                "weight": 0.05
            },
            # Background setting
            {
                "command": "set_background",
                "params": {
                    "color": "colors"
                },
                "weight": 0.08
            }
        ]
    
    def generate_random_dsl(self, num_commands: int = 5) -> str:
        """
        Generate complex random DSL commands.
        
        Args:
            num_commands: Number of commands to generate
            
        Returns:
            Generated DSL commands as string
        """
        commands = []
        
        # Always start with background
        bg_color = random.choice(self.colors)
        commands.append(f'set_background(color="{bg_color}")')
        
        # Generate main commands
        for _ in range(num_commands - 1):  # -1 because we already added background
            # Select command based on weights
            template = random.choices(
                self.dsl_templates,
                weights=[t["weight"] for t in self.dsl_templates]
            )[0]
            
            # Skip background command if already added
            if template["command"] == "set_background":
                continue
            
            # Generate parameters
            params = []
            for param_name, param_config in template["params"].items():
                if param_config == "colors":
                    value = f'"{random.choice(self.colors)}"'
                elif isinstance(param_config, tuple):
                    min_val, max_val = param_config
                    value = str(random.randint(min_val, max_val))
                else:
                    value = str(param_config)
                
                params.append(f"{param_name}={value}")
            
            # Build command
            command = f"{template['command']}({', '.join(params)})"
            commands.append(command)
        
        # Add some overlapping shapes for complexity
        if random.random() < 0.4:  # 40% chance to add overlapping elements
            overlap_commands = self._generate_overlapping_elements()
            commands.extend(overlap_commands)
        
        # Add some small detail elements
        if random.random() < 0.3:  # 30% chance to add details
            detail_commands = self._generate_detail_elements()
            commands.extend(detail_commands)
        
        return '\n'.join(commands)
    
    def _generate_overlapping_elements(self) -> List[str]:
        """Generate overlapping shapes for complexity."""
        commands = []
        
        # Choose a center point
        center_x = random.randint(150, 362)
        center_y = random.randint(150, 362)
        
        # Generate 2-4 overlapping shapes
        for _ in range(random.randint(2, 4)):
            shape_type = random.choice(["fill_circle", "fill_rect", "fill_ellipse"])
            color = random.choice(self.colors)
            
            # Vary position around center
            x = center_x + random.randint(-50, 50)
            y = center_y + random.randint(-50, 50)
            
            if shape_type == "fill_circle":
                radius = random.randint(15, 60)
                commands.append(f'{shape_type}(x={x}, y={y}, radius={radius}, color="{color}")')
            elif shape_type == "fill_rect":
                w = random.randint(20, 80)
                h = random.randint(20, 80)
                commands.append(f'{shape_type}(x={x}, y={y}, w={w}, h={h}, color="{color}")')
            else:  # fill_ellipse
                w = random.randint(20, 70)
                h = random.randint(20, 70)
                commands.append(f'{shape_type}(x={x}, y={y}, w={w}, h={h}, color="{color}")')
        
        return commands
    
    def _generate_detail_elements(self) -> List[str]:
        """Generate small detail elements."""
        commands = []
        
        # Add 1-3 small detail elements
        for _ in range(random.randint(1, 3)):
            detail_type = random.choice(["draw_circle", "draw_rect", "draw_line"])
            color = random.choice(self.colors)
            
            if detail_type == "draw_circle":
                x = random.randint(20, 492)
                y = random.randint(20, 492)
                radius = random.randint(3, 20)
                commands.append(f'{detail_type}(x={x}, y={y}, radius={radius}, color="{color}")')
            elif detail_type == "draw_rect":
                x = random.randint(20, 450)
                y = random.randint(20, 450)
                w = random.randint(5, 30)
                h = random.randint(5, 30)
                commands.append(f'{detail_type}(x={x}, y={y}, w={w}, h={h}, color="{color}")')
            else:  # draw_line
                x1 = random.randint(20, 492)
                y1 = random.randint(20, 492)
                x2 = random.randint(20, 492)
                y2 = random.randint(20, 492)
                width = random.randint(1, 5)
                commands.append(f'{detail_type}(x1={x1}, y1={y1}, x2={x2}, y2={y2}, color="{color}", width={width})')
        
        return commands
    
    def explore_and_learn(self, num_explorations: int = 10, confidence_threshold: float = 0.7) -> List[Dict[str, Any]]:
        """
        Perform exploratory learning using BLIP's confidence scores.
        
        Args:
            num_explorations: Number of explorations to perform
            confidence_threshold: Minimum BLIP confidence to consider an exploration successful
            
        Returns:
            List of successful explorations
        """
        successful_explorations = []
        
        self.logger.info(f"Starting exploratory learning with {num_explorations} explorations")
        self.logger.info(f"Confidence threshold: {confidence_threshold}")
        
        # Ensure output directory exists
        output_dir = self.config.generator.output_dir
        os.makedirs(output_dir, exist_ok=True)
        
        for i in range(num_explorations):
            self.logger.info(f"Exploration {i+1}/{num_explorations}")
            
            try:
                # Generate complex random DSL commands
                num_commands = random.randint(4, 8)  # Vary complexity
                dsl_commands = self.generate_random_dsl(num_commands)
                
                self.logger.debug(f"Generated random DSL ({num_commands} commands): {dsl_commands}")
                
                # Generate image
                image = self.generator.generate(dsl_commands)
                
                # Save exploration image with timestamp
                timestamp = datetime.datetime.now().strftime("%Y%m%d_%H%M%S_%f")[:-3]  # Include milliseconds
                image_path = os.path.join(output_dir, f"exploration_{timestamp}_{i+1}.png")
                image.save(image_path)
                self.logger.info(f"Exploration image saved to: {image_path}")
                
                # Get BLIP description with confidence
                description, blip_confidence = self.descriptor.describe_with_confidence(image)
                
                self.logger.info(f"BLIP description: {description} (confidence: {blip_confidence:.3f})")
                
                # Use BLIP's confidence directly
                if blip_confidence >= confidence_threshold:
                    # Create a synthetic input text based on the description
                    synthetic_input = self._create_synthetic_input(description, dsl_commands)
                    
                    # Add to examples with exploration flag
                    example_data = {
                        "input": synthetic_input,
                        "dsl": dsl_commands,
                        "description": description,
                        "from_exploration": True,
                        "blip_confidence": blip_confidence,
                        "num_commands": num_commands,
                        "image_path": image_path
                    }
                    
                    self.example_loader.examples.append(example_data)
                    self.example_loader._save_examples()
                    
                    exploration_result = {
                        "exploration_id": i + 1,
                        "dsl_commands": dsl_commands,
                        "description": description,
                        "blip_confidence": blip_confidence,
                        "synthetic_input": synthetic_input,
                        "num_commands": num_commands,
                        "image_path": image_path,
                        "success": True
                    }
                    
                    successful_explorations.append(exploration_result)
                    self.logger.info(f"Added confident example: {synthetic_input[:50]}... (BLIP confidence: {blip_confidence:.3f})")
                else:
                    self.logger.info(f"BLIP confidence {blip_confidence:.3f} below threshold {confidence_threshold}, skipping")
                
            except Exception as e:
                self.logger.error(f"Error in exploration {i+1}: {e}")
                continue
        
        self.logger.info(f"Exploratory learning completed. {len(successful_explorations)} successful explorations")
        return successful_explorations
    
    def _create_synthetic_input(self, description: str, dsl_commands: str) -> str:
        """
        Create synthetic input text based on BLIP description and DSL commands.
        
        Args:
            description: BLIP description
            dsl_commands: Original DSL commands
            
        Returns:
            Synthetic input text
        """
        # Extract key elements from description
        description_lower = description.lower()
        
        # Find colors mentioned
        colors_found = [color for color in self.colors if color in description_lower]
        
        # Find shapes mentioned
        shapes_found = []
        if "circle" in description_lower:
            shapes_found.append("circle")
        if "rectangle" in description_lower or "square" in description_lower:
            shapes_found.append("rectangle")
        if "line" in description_lower:
            shapes_found.append("line")
        if "triangle" in description_lower:
            shapes_found.append("triangle")
        if "ellipse" in description_lower or "oval" in description_lower:
            shapes_found.append("ellipse")
        
        # Create synthetic input
        parts = []
        
        if colors_found:
            parts.append(f"a {colors_found[0]}")
        
        if shapes_found:
            parts.append(shapes_found[0])
        else:
            parts.append("shape")
        
        if len(parts) >= 2:
            synthetic_input = f"{parts[0]} {parts[1]}"
        else:
            synthetic_input = "a geometric shape"
        
        return synthetic_input
    
    def get_exploration_stats(self) -> Dict[str, Any]:
        """Get statistics about exploratory learning."""
        examples = self.example_loader.get_examples()
        
        # Count examples from exploration
        exploration_examples = [ex for ex in examples if ex.get("from_exploration", False)]
        
        return {
            "total_examples": len(examples),
            "exploration_examples": len(exploration_examples),
            "exploration_ratio": len(exploration_examples) / len(examples) if examples else 0
        } 
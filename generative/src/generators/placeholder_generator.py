"""Placeholder image generator for DSL commands."""

from PIL import Image, ImageDraw, ImageFont
from typing import Any, Optional
import os
import re

from .base_generator import BaseGenerator
from ..utils.config import get_config
from ..utils.logging import LoggerMixin

class PlaceholderGenerator(BaseGenerator, LoggerMixin):
    """A placeholder generator that renders DSL commands as simple shapes on an image."""
    
    def __init__(self):
        self.config = get_config()
        gen_cfg = self.config.generator
        self.image_size = tuple(gen_cfg.image_size)
        self.bg_color = tuple(gen_cfg.placeholder.get("background_color", [255, 255, 255]))
        self.text_color = tuple(gen_cfg.placeholder.get("text_color", [0, 0, 0]))
        self.font_size = gen_cfg.placeholder.get("font_size", 24)
        self.padding = gen_cfg.placeholder.get("padding", 20)
        self.font = self._load_font()
        self.logger.info(f"Initialized PlaceholderGenerator with size {self.image_size}")
    
    def _load_font(self) -> ImageFont.ImageFont:
        try:
            return ImageFont.truetype("arial.ttf", self.font_size)
        except Exception:
            return ImageFont.load_default()
    
    def generate(self, dsl_commands: str) -> Image.Image:
        """
        Generate a placeholder image with DSL commands as shapes only.
        
        Args:
            dsl_commands: DSL command string
            
        Returns:
            PIL Image with the DSL commands rendered as shapes
        """
        self.logger.info(f"Generating placeholder image for DSL: {dsl_commands}")
        img = Image.new("RGB", self.image_size, self.bg_color)
        draw = ImageDraw.Draw(img)
        
        # Split DSL commands into lines
        lines = dsl_commands.split("\n")
        for line in lines:
            line = line.strip()
            if not line:
                continue
            if line.startswith("draw_rect"):
                self._draw_rect(draw, line)
            elif line.startswith("draw_circle"):
                self._draw_circle(draw, line)
            # No text rendering for unrecognized commands
        return img

    def _parse_args(self, line: str) -> dict:
        # Parse command arguments like draw_rect(x=10, y=20, w=100, h=50, color="red")
        m = re.search(r'\((.*)\)', line)
        if not m:
            return {}
        args_str = m.group(1)
        args = {}
        for part in args_str.split(','):
            if '=' in part:
                k, v = part.split('=', 1)
                k = k.strip()
                v = v.strip().strip('"')
                # Try to convert to int or tuple
                if k in {'x', 'y', 'w', 'h', 'r', 'g', 'b', 'radius'}:
                    try:
                        v = int(v)
                    except Exception:
                        pass
                elif k == 'color':
                    # Accept color as name or tuple
                    if v.startswith('(') and v.endswith(')'):
                        v = tuple(map(int, v[1:-1].split(',')))
                args[k] = v
        return args

    def _draw_rect(self, draw: ImageDraw.ImageDraw, line: str):
        args = self._parse_args(line)
        x = args.get('x', 10)
        y = args.get('y', 10)
        w = args.get('w', 100)
        h = args.get('h', 50)
        color = args.get('color', (200, 0, 0))
        if isinstance(color, str):
            color = color if color else 'red'
        draw.rectangle([x, y, x + w, y + h], outline=color, width=3)

    def _draw_circle(self, draw: ImageDraw.ImageDraw, line: str):
        args = self._parse_args(line)
        x = args.get('x', 60)
        y = args.get('y', 60)
        radius = args.get('radius', 40)
        color = args.get('color', (0, 0, 200))
        if isinstance(color, str):
            color = color if color else 'blue'
        bbox = [x - radius, y - radius, x + radius, y + radius]
        draw.ellipse(bbox, outline=color, width=3) 
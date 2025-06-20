"""Configuration management for the generative AI system."""

import os
import yaml
from pathlib import Path
from typing import Any, Dict, List, Optional, Union
from dataclasses import dataclass, field
from pydantic import BaseModel, Field


class ModelConfig(BaseModel):
    """Configuration for AI models."""
    model_name: str
    max_length: int = 512
    temperature: float = 0.7
    top_p: float = 0.9
    do_sample: bool = True
    device: str = "auto"


class MistralConfig(ModelConfig):
    """Configuration for Mistral model."""
    pass


class BLIPConfig(ModelConfig):
    """Configuration for BLIP model."""
    max_length: int = 100


class VectorizerConfig(BaseModel):
    """Configuration for text vectorizer."""
    model_name: str = "sentence-transformers/all-MiniLM-L6-v2"
    similarity_metric: str = "cosine"
    device: str = "auto"


class GeneratorConfig(BaseModel):
    """Configuration for image generators."""
    type: str = "placeholder"
    output_dir: str = "data/outputs"
    image_size: List[int] = Field(default=[512, 512])
    format: str = "PNG"
    placeholder: Dict[str, Any] = Field(default_factory=dict)


class LearningConfig(BaseModel):
    """Configuration for learning system."""
    similarity_threshold: float = 0.8
    max_examples_per_rule: int = 100
    min_confidence: float = 0.7
    learning_rate: float = 0.1
    batch_size: int = 32
    rules: List[Dict[str, Any]] = Field(default_factory=list)


class APIConfig(BaseModel):
    """Configuration for API server."""
    host: str = "0.0.0.0"
    port: int = 8000
    debug: bool = False
    workers: int = 1
    rate_limit: Dict[str, int] = Field(default_factory=dict)


class LoggingConfig(BaseModel):
    """Configuration for logging."""
    level: str = "INFO"
    format: str = "%(asctime)s - %(name)s - %(levelname)s - %(message)s"
    file: str = "logs/generative_ai.log"
    max_size: str = "10MB"
    backup_count: int = 5


class DataConfig(BaseModel):
    """Configuration for data management."""
    examples_dir: str = "data/examples"
    rules_dir: str = "data/rules"
    outputs_dir: str = "data/outputs"
    cache_dir: str = "data/cache"
    max_examples_age_days: int = 30
    max_outputs_age_days: int = 7
    cleanup_interval_hours: int = 24


class DSLCommand(BaseModel):
    """Configuration for DSL commands."""
    name: str
    description: str
    parameters: List[str]


class DSLConfig(BaseModel):
    """Configuration for Domain Specific Language."""
    commands: List[DSLCommand] = Field(default_factory=list)


class PerformanceConfig(BaseModel):
    """Configuration for performance optimization."""
    use_cache: bool = True
    cache_size: int = 1000
    parallel_processing: bool = True
    max_workers: int = 4
    max_memory_usage_gb: int = 8
    cleanup_threshold: float = 0.8


class Config(BaseModel):
    """Main configuration class."""
    models: Dict[str, ModelConfig] = Field(default_factory=dict)
    generator: GeneratorConfig = Field(default_factory=GeneratorConfig)
    learning: LearningConfig = Field(default_factory=LearningConfig)
    api: APIConfig = Field(default_factory=APIConfig)
    logging: LoggingConfig = Field(default_factory=LoggingConfig)
    data: DataConfig = Field(default_factory=DataConfig)
    dsl: DSLConfig = Field(default_factory=DSLConfig)
    performance: PerformanceConfig = Field(default_factory=PerformanceConfig)

    @classmethod
    def from_file(cls, config_path: Union[str, Path]) -> "Config":
        """Load configuration from YAML file."""
        config_path = Path(config_path)
        
        if not config_path.exists():
            raise FileNotFoundError(f"Configuration file not found: {config_path}")
        
        with open(config_path, 'r', encoding='utf-8') as f:
            config_data = yaml.safe_load(f)
        
        return cls(**config_data)
    
    @classmethod
    def from_env(cls) -> "Config":
        """Load configuration from environment variables."""
        config_path = os.getenv("GENERATIVE_CONFIG", "config/config.yaml")
        return cls.from_file(config_path)
    
    def save(self, config_path: Union[str, Path]) -> None:
        """Save configuration to YAML file."""
        config_path = Path(config_path)
        config_path.parent.mkdir(parents=True, exist_ok=True)
        
        with open(config_path, 'w', encoding='utf-8') as f:
            yaml.dump(self.dict(), f, default_flow_style=False, indent=2)
    
    def get_model_config(self, model_type: str) -> Optional[ModelConfig]:
        """Get configuration for a specific model type."""
        return self.models.get(model_type)
    
    def get_dsl_command(self, command_name: str) -> Optional[DSLCommand]:
        """Get DSL command configuration by name."""
        for command in self.dsl.commands:
            if command.name == command_name:
                return command
        return None


# Global configuration instance
_config: Optional[Config] = None


def get_config() -> Config:
    """Get the global configuration instance."""
    global _config
    if _config is None:
        _config = Config.from_env()
    return _config


def set_config(config: Config) -> None:
    """Set the global configuration instance."""
    global _config
    _config = config 
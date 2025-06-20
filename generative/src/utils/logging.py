"""Logging utilities for the generative AI system."""

import logging
import logging.handlers
import sys
from pathlib import Path
from typing import Optional

from .config import get_config, LoggingConfig


def setup_logging(
    name: str = "generative_ai",
    level: Optional[str] = None,
    log_file: Optional[str] = None,
    format_string: Optional[str] = None
) -> logging.Logger:
    """
    Set up logging configuration.
    
    Args:
        name: Logger name
        level: Logging level (DEBUG, INFO, WARNING, ERROR, CRITICAL)
        log_file: Path to log file
        format_string: Log format string
        
    Returns:
        Configured logger instance
    """
    config = get_config()
    logging_config = config.logging
    
    # Use provided parameters or fall back to config
    level = level or logging_config.level
    log_file = log_file or logging_config.file
    format_string = format_string or logging_config.format
    
    # Create logger
    logger = logging.getLogger(name)
    logger.setLevel(getattr(logging, level.upper()))
    
    # Clear existing handlers
    logger.handlers.clear()
    
    # Create formatter
    formatter = logging.Formatter(format_string)
    
    # Console handler
    console_handler = logging.StreamHandler(sys.stdout)
    console_handler.setLevel(getattr(logging, level.upper()))
    console_handler.setFormatter(formatter)
    logger.addHandler(console_handler)
    
    # File handler with rotation
    if log_file:
        log_path = Path(log_file)
        log_path.parent.mkdir(parents=True, exist_ok=True)
        
        # Parse max_size (e.g., "10MB" -> 10 * 1024 * 1024)
        max_size_str = logging_config.max_size
        if max_size_str.endswith("MB"):
            max_size = int(max_size_str[:-2]) * 1024 * 1024
        elif max_size_str.endswith("KB"):
            max_size = int(max_size_str[:-2]) * 1024
        else:
            max_size = int(max_size_str)
        
        file_handler = logging.handlers.RotatingFileHandler(
            log_path,
            maxBytes=max_size,
            backupCount=logging_config.backup_count
        )
        file_handler.setLevel(getattr(logging, level.upper()))
        file_handler.setFormatter(formatter)
        logger.addHandler(file_handler)
    
    return logger


def get_logger(name: str = "generative_ai") -> logging.Logger:
    """
    Get a logger instance.
    
    Args:
        name: Logger name
        
    Returns:
        Logger instance
    """
    return logging.getLogger(name)


class LoggerMixin:
    """Mixin class to add logging capabilities to any class."""
    
    @property
    def logger(self) -> logging.Logger:
        """Get logger for this class."""
        return get_logger(f"{self.__class__.__module__}.{self.__class__.__name__}")


def log_function_call(logger: logging.Logger):
    """
    Decorator to log function calls.
    
    Args:
        logger: Logger instance to use
    """
    def decorator(func):
        def wrapper(*args, **kwargs):
            logger.debug(f"Calling {func.__name__} with args={args}, kwargs={kwargs}")
            try:
                result = func(*args, **kwargs)
                logger.debug(f"{func.__name__} returned {result}")
                return result
            except Exception as e:
                logger.error(f"{func.__name__} raised {type(e).__name__}: {e}")
                raise
        return wrapper
    return decorator


def log_execution_time(logger: logging.Logger):
    """
    Decorator to log function execution time.
    
    Args:
        logger: Logger instance to use
    """
    import time
    
    def decorator(func):
        def wrapper(*args, **kwargs):
            start_time = time.time()
            logger.debug(f"Starting {func.__name__}")
            
            try:
                result = func(*args, **kwargs)
                execution_time = time.time() - start_time
                logger.info(f"{func.__name__} completed in {execution_time:.2f}s")
                return result
            except Exception as e:
                execution_time = time.time() - start_time
                logger.error(f"{func.__name__} failed after {execution_time:.2f}s: {e}")
                raise
        return wrapper
    return decorator 
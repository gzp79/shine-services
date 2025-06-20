"""Similarity checking utilities for generative learning."""

from ..models.vectorizer import TextVectorizer
from ..utils.config import get_config
from ..utils.logging import LoggerMixin

class SimilarityChecker(LoggerMixin):
    """Checks similarity between input and output descriptions."""
    def __init__(self):
        self.config = get_config()
        self.vectorizer = TextVectorizer()
        self.threshold = self.config.learning.similarity_threshold
    
    def is_similar(self, input_text: str, output_text: str) -> bool:
        score = self.vectorizer.calculate_similarity(input_text, output_text)
        self.logger.info(f"Similarity score: {score:.3f} (threshold: {self.threshold})")
        return score >= self.threshold
    
    def similarity_score(self, input_text: str, output_text: str) -> float:
        return self.vectorizer.calculate_similarity(input_text, output_text) 
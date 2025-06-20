"""Main orchestrator for the generative auto-learning loop."""

from models.mistral_dsl import MistralDSLConverter
from generators.placeholder_generator import PlaceholderGenerator
from models.blip_descriptor import BLIPDescriptor
from models.vectorizer import TextVectorizer
from learning.rule_manager import RuleManager
from utils.config import get_config
from utils.logging import setup_logging

from PIL import Image
import os
import uuid


def run_auto_learning(input_texts):
    config = get_config()
    logger = setup_logging()
    dsl_converter = MistralDSLConverter()
    generator = PlaceholderGenerator()
    descriptor = BLIPDescriptor()
    vectorizer = TextVectorizer()
    rule_manager = RuleManager()

    output_dir = config.generator.output_dir
    os.makedirs(output_dir, exist_ok=True)

    for input_text in input_texts:
        logger.info(f"\n=== Processing: {input_text} ===")
        # 1. Convert text to DSL
        dsl_commands = dsl_converter.convert(input_text)
        logger.info(f"DSL: {dsl_commands}")

        # 2. Generate image from DSL
        image = generator.generate(dsl_commands)
        image_id = str(uuid.uuid4())
        image_path = os.path.join(output_dir, f"{image_id}.png")
        image.save(image_path)
        logger.info(f"Image saved to: {image_path}")

        # 3. Describe image with BLIP-2
        description = descriptor.describe(image)
        logger.info(f"BLIP-2 Description: {description}")

        # 4. Vectorize and compare input/description
        similarity = vectorizer.calculate_similarity(input_text, description)
        logger.info(f"Input/Output Similarity: {similarity:.3f}")

        # 5. If similar, add as example
        rule_manager.evaluate_and_learn(input_text, dsl_commands, description)

        logger.info(f"=== Done: {input_text} ===\n")

if __name__ == "__main__":
    # Example usage: replace with your own input list or CLI
    test_inputs = [
        "A red cat sitting on a blue chair",
        "A green tree under a yellow sun",
        "A small house with a red roof and a garden"
    ]
    run_auto_learning(test_inputs) 
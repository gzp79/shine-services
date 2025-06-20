"""Main orchestrator for the generative auto-learning loop."""

from src.models.mistral_dsl import MistralDSLConverter
from src.generators.placeholder_generator import PlaceholderGenerator
from src.models.blip_descriptor import BLIPDescriptor
from src.models.vectorizer import TextVectorizer
from src.learning.rule_manager import RuleManager
from src.learning.example_loader import ExampleLoader
from src.learning.exploratory_learner import ExploratoryLearner
from src.utils.config import get_config
from src.utils.logging import setup_logging

from PIL import Image
import os
import datetime


def run_auto_learning(input_texts, enable_exploration=True, exploration_interval=5, confidence_threshold=0.7):
    """
    Run the auto-learning loop with optional exploratory learning.
    
    Args:
        input_texts: List of input texts to process
        enable_exploration: Whether to enable exploratory learning
        exploration_interval: Run exploration every N inputs
        confidence_threshold: Minimum BLIP confidence for exploration discoveries
    """
    config = get_config()
    logger = setup_logging()
    dsl_converter = MistralDSLConverter()
    generator = PlaceholderGenerator()
    descriptor = BLIPDescriptor()
    vectorizer = TextVectorizer()
    rule_manager = RuleManager()
    example_loader = ExampleLoader()
    
    # Initialize exploratory learner if enabled
    exploratory_learner = None
    if enable_exploration:
        exploratory_learner = ExploratoryLearner()
        logger.info("Exploratory learning enabled")

    output_dir = config.generator.output_dir
    os.makedirs(output_dir, exist_ok=True)

    similarity_threshold = getattr(config.learning, 'similarity_threshold', 0.8)

    for i, input_text in enumerate(input_texts):
        logger.info(f"\n=== Processing: {input_text} ===")
        
        # Run exploratory learning periodically
        if enable_exploration and exploratory_learner and (i + 1) % exploration_interval == 0:
            logger.info(f"\n--- Running exploratory learning (iteration {i + 1}) ---")
            try:
                exploration_results = exploratory_learner.explore_and_learn(
                    num_explorations=5,  # Small batch for regular exploration
                    confidence_threshold=confidence_threshold
                )
                logger.info(f"Exploration completed: {len(exploration_results)} successful discoveries")
                
                # Show exploration stats
                stats = exploratory_learner.get_exploration_stats()
                logger.info(f"Exploration stats: {stats['exploration_examples']}/{stats['total_examples']} examples from exploration")
                
            except Exception as e:
                logger.error(f"Exploration failed: {e}")
        
        # 1. Convert text to DSL with retries
        dsl_commands = dsl_converter.convert(input_text, max_retries=3)
        
        if dsl_commands is None:
            logger.warning(f"Failed to generate DSL commands for: {input_text}")
            logger.info(f"=== Skipped: {input_text} ===\n")
            continue
        
        logger.info(f"DSL: {dsl_commands}")

        # 2. Generate image from DSL
        image = generator.generate(dsl_commands)
        timestamp = datetime.datetime.now().strftime("%Y%m%d_%H%M%S_%f")[:-3]  # Include milliseconds
        image_path = os.path.join(output_dir, f"image_{timestamp}.png")
        image.save(image_path)
        logger.info(f"Image saved to: {image_path}")

        # 3. Describe image with BLIP-2 (with confidence)
        description, blip_confidence = descriptor.describe_with_confidence(image)
        logger.info(f"BLIP-2 Description: {description} (confidence: {blip_confidence:.3f})")

        # 4. Vectorize and compare input/description
        similarity = vectorizer.calculate_similarity(input_text, description)
        logger.info(f"Input/Output Similarity: {similarity:.3f}")

        # 5. If similar, add as example and evaluate learning
        if similarity >= similarity_threshold:
            # Add to examples for future learning
            example_loader.add_example(input_text, dsl_commands, description)
            logger.info(f"Added example for: {input_text[:50]}...")
            
            # Evaluate learning process
            rule_manager.evaluate_and_learn(input_text, dsl_commands, description)
        else:
            logger.info(f"Result not similar enough (threshold: {similarity_threshold}), not storing example.")

        logger.info(f"=== Done: {input_text} ===\n")

    # Final exploration session
    if enable_exploration and exploratory_learner:
        logger.info(f"\n--- Final exploratory learning session ---")
        try:
            exploration_results = exploratory_learner.explore_and_learn(
                num_explorations=10,  # Larger batch for final exploration
                confidence_threshold=confidence_threshold
            )
            logger.info(f"Final exploration completed: {len(exploration_results)} successful discoveries")
            
            # Show final stats
            stats = exploratory_learner.get_exploration_stats()
            logger.info(f"Final exploration stats: {stats['exploration_examples']}/{stats['total_examples']} examples from exploration")
            
        except Exception as e:
            logger.error(f"Final exploration failed: {e}")


def run_exploration_only(num_explorations=20, confidence_threshold=0.7):
    """
    Run only exploratory learning without processing input texts.
    
    Args:
        num_explorations: Number of explorations to perform
        confidence_threshold: Minimum BLIP confidence for discoveries
    """
    config = get_config()
    logger = setup_logging()
    
    logger.info(f"Starting exploration-only session with {num_explorations} explorations")
    logger.info(f"Confidence threshold: {confidence_threshold}")
    
    exploratory_learner = ExploratoryLearner()
    
    try:
        exploration_results = exploratory_learner.explore_and_learn(
            num_explorations=num_explorations,
            confidence_threshold=confidence_threshold
        )
        
        logger.info(f"Exploration completed: {len(exploration_results)} successful discoveries")
        
        # Show detailed results
        for result in exploration_results:
            logger.info(f"Discovery {result['exploration_id']}: {result['synthetic_input']} (BLIP confidence: {result['blip_confidence']:.3f})")
        
        # Show final stats
        stats = exploratory_learner.get_exploration_stats()
        logger.info(f"Final stats: {stats['exploration_examples']}/{stats['total_examples']} examples from exploration")
        
    except Exception as e:
        logger.error(f"Exploration failed: {e}")


if __name__ == "__main__":
    # Example usage: replace with your own input list or CLI
    test_inputs = [
        "A red cat sitting on a blue chair",
        "A green tree under a yellow sun",
        "A small house with a red roof and a garden",
        "A purple circle in the center",
        "A black line from left to right"
    ]
    
    # Run with exploratory learning enabled
    run_auto_learning(test_inputs, enable_exploration=True, exploration_interval=3, confidence_threshold=0.6)
    
    # Uncomment to run exploration-only mode
    # run_exploration_only(num_explorations=15, confidence_threshold=0.6) 
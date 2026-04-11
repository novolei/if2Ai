"""
Test fixtures for Harness framework.

Provides fixtures for creating test agents, tools, and LLM providers.
"""

from dataclasses import dataclass
from typing import Any, Dict, List, Optional


@dataclass
class MockTool:
    """Mock tool for testing."""
    name: str
    description: str = ""
    inputs: Dict[str, Any] = None
    outputs: Dict[str, str] = None
    
    def __post_init__(self):
        if self.inputs is None:
            self.inputs = {}
        if self.outputs is None:
            self.outputs = {}
    
    def execute(self, input_keys: Dict[str, Any]) -> str:
        """Execute the mock tool."""
        # Simple string matching for demo
        for key, expected_output in self.outputs.items():
            if str(input_keys).lower().find(key.lower()) >= 0:
                return expected_output
        return "Tool execution result"


@dataclass
class MockLLMProvider:
    """Mock LLM provider for deterministic testing."""
    name: str = "mock"
    responses: Dict[str, str] = None
    default_response: str = "This is a mock response"
    
    def __post_init__(self):
        if self.responses is None:
            self.responses = {}
    
    async def complete(self, prompt: str, **kwargs) -> str:
        """Return a mocked completion."""
        # Look for matching prompt
        for key, response in self.responses.items():
            if key.lower() in prompt.lower():
                return response
        return self.default_response
    
    async def chat(self, messages: List[Dict[str, str]], **kwargs) -> str:
        """Return a mocked chat response."""
        # Simple implementation
        if messages:
            last_message = messages[-1].get("content", "")
            return self.complete(last_message, **kwargs)
        return self.default_response


@dataclass
class AgentFixture:
    """Fixture for creating test agents."""
    name: str
    tools: List[str] = None
    llm_provider: str = "mock"
    config: Dict[str, Any] = None
    
    def __post_init__(self):
        if self.tools is None:
            self.tools = []
        if self.config is None:
            self.config = {}


class ToolFixture:
    """Fixture collection for tools."""
    
    @staticmethod
    def calculator() -> MockTool:
        """Calculator tool fixture."""
        return MockTool(
            name="calculator",
            description="Performs basic math operations",
            inputs={"expression": "str"},
            outputs={
                "2+2": "4",
                "3*4": "12",
                "10-5": "5",
                "20/4": "5",
            }
        )
    
    @staticmethod
    def search() -> MockTool:
        """Search tool fixture."""
        return MockTool(
            name="search",
            description="Searches for information",
            inputs={"query": "str"},
            outputs={
                "capital of france": "Paris",
                "capital of italy": "Rome",
                "capital of spain": "Madrid",
            }
        )
    
    @staticmethod
    def get_weather() -> MockTool:
        """Weather tool fixture."""
        return MockTool(
            name="get_weather",
            description="Gets current weather",
            inputs={"location": "str"},
            outputs={
                "paris": "Sunny, 20°C",
                "london": "Cloudy, 15°C",
                "new york": "Rainy, 18°C",
            }
        )


class LLMFixture:
    """Fixture collection for LLM providers."""
    
    @staticmethod
    def basic_math() -> MockLLMProvider:
        """LLM for math problems."""
        return MockLLMProvider(
            name="math_llm",
            responses={
                "2+2": "The answer is 4",
                "3*4": "The answer is 12",
                "calculate": "I should use the calculator tool",
            }
        )
    
    @staticmethod
    def answer_questions() -> MockLLMProvider:
        """LLM for answering questions."""
        return MockLLMProvider(
            name="qa_llm",
            responses={
                "capital": "I should search for the capital",
                "weather": "I should check the weather",
            }
        )


__all__ = [
    "MockTool",
    "MockLLMProvider",
    "AgentFixture",
    "ToolFixture",
    "LLMFixture",
]

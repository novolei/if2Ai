"""
Local test runner implementation.
"""

from harness import BaseRunner, ExecutionResult, ExecutionMetrics, TestCase, ExecutionEvent, ExecutionEventType
from harness.fixtures import MockLLMProvider, AgentFixture
from datetime import datetime
import asyncio
from typing import Optional


class LocalRunner(BaseRunner):
    """Runs Agent tests locally."""
    
    def __init__(self, agent_fixture: Optional[AgentFixture] = None, **config):
        """
        Initialize local runner.
        
        Args:
            agent_fixture: The agent fixture to use
            **config: Additional configuration
        """
        super().__init__("local_runner", **config)
        self.agent_fixture = agent_fixture
    
    async def run(self, test_case: TestCase) -> ExecutionResult:
        """
        Run a test case locally.
        
        Args:
            test_case: The test case to run
            
        Returns:
            ExecutionResult with mock output
        """
        run_id = f"run-{datetime.now().isoformat()}"
        events = []
        start_time = datetime.now()
        
        try:
            # Event: Execution started
            events.append(ExecutionEvent(
                type=ExecutionEventType.INITIALIZED,
                timestamp=datetime.now(),
                details={"test_case": test_case.name}
            ))
            
            # Event: Prompt sent
            events.append(ExecutionEvent(
                type=ExecutionEventType.PROMPT_SENT,
                timestamp=datetime.now(),
                details={"prompt": test_case.prompt}
            ))
            
            # Simulate LLM call
            await asyncio.sleep(0.1)  # Simulate some processing
            llm_provider = self._get_llm_provider()
            output = await llm_provider.complete(test_case.prompt)
            
            # Event: LLM response
            events.append(ExecutionEvent(
                type=ExecutionEventType.LLM_RESPONSE,
                timestamp=datetime.now(),
                details={"output": output}
            ))
            
            # Event: Completion
            events.append(ExecutionEvent(
                type=ExecutionEventType.COMPLETION,
                timestamp=datetime.now(),
                details={"success": True}
            ))
            
            # Calculate metrics
            duration = (datetime.now() - start_time).total_seconds()
            metrics = ExecutionMetrics(
                total_tokens=len(test_case.prompt.split()) + len(output.split()),
                prompt_tokens=len(test_case.prompt.split()),
                completion_tokens=len(output.split()),
                duration_seconds=duration,
                tool_calls=0,
                error_count=0,
            )
            
            return ExecutionResult(
                run_id=run_id,
                prompt=test_case.prompt,
                output=output,
                success=True,
                events=events,
                metrics=metrics,
            )
            
        except asyncio.TimeoutError:
            return ExecutionResult(
                run_id=run_id,
                prompt=test_case.prompt,
                success=False,
                error="Execution timeout",
                events=events,
                metrics=ExecutionMetrics(error_count=1, errors=["Timeout"]),
            )
        except Exception as e:
            return ExecutionResult(
                run_id=run_id,
                prompt=test_case.prompt,
                success=False,
                error=str(e),
                events=events,
                metrics=ExecutionMetrics(
                    error_count=1, 
                    errors=[str(e)]
                ),
            )
    
    def _get_llm_provider(self) -> MockLLMProvider:
        """Get LLM provider based on fixture."""
        if self.agent_fixture and self.agent_fixture.llm_provider:
            # In a real implementation, would load based on provider name
            return MockLLMProvider()
        return MockLLMProvider()


__all__ = ["LocalRunner"]

import { Card, Title, Text, Button, Badge, Group, Stack, Box } from "@mantine/core";
import { CodeHighlight } from "@mantine/code-highlight";
import { useState } from "react";
import type { ReactNode } from "react";
import { safeExecute, globalPerformanceMonitor } from "../utils/safeExecution";
import { ErrorBoundary } from "./ErrorBoundary";

interface ExampleCardProps {
  title: string;
  description: string;
  code: string;
  result?: string;
  onRun?: () => Promise<string>;
  category?: string;
  badge?: ReactNode;
  timeout?: number;
  retries?: number;
}

export function ExampleCard({
  title,
  description,
  code,
  result: initialResult,
  onRun,
  category,
  badge,
  timeout = 10000,
  retries = 1
}: ExampleCardProps) {
  const [result, setResult] = useState(initialResult);
  const [isRunning, setIsRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [performanceInfo, setPerformanceInfo] = useState<string | null>(null);

  const handleRun = async () => {
    if (!onRun) return;

    setIsRunning(true);
    setError(null);
    setPerformanceInfo(null);

    const executionResult = await safeExecute(
      async () => {
        const { result: output, duration } = await globalPerformanceMonitor.measure(
          `example-${title}`,
          onRun
        );
        return { output, duration };
      },
      {
        timeout,
        retries,
        fallback: () => ({ output: "Execution timed out or failed, but this is expected behavior for demonstration.", duration: 0 })
      }
    );

    setIsRunning(false);

    if (executionResult.success && executionResult.data) {
      setResult(executionResult.data.output);
      if (executionResult.data.duration > 0) {
        setPerformanceInfo(`Execution time: ${executionResult.data.duration.toFixed(2)}ms`);
      }
      if (executionResult.fallbackUsed) {
        setPerformanceInfo(prev => `${prev || ''} (fallback used)`.trim());
      }
    } else {
      setError(executionResult.error || 'Unknown error occurred');
    }
  };

  return (
    <ErrorBoundary>
      <Card withBorder mb="lg" w="100%">
        <Card.Section withBorder inheritPadding py="sm">
          <Group justify="space-between" align="flex-start">
            <Box>
              <Title order={4}>{title}</Title>
              <Group gap="xs" mt={4}>
                {category && (
                  <Badge variant="light" color="blue" size="sm">
                    {category}
                  </Badge>
                )}
                {badge}
                {performanceInfo && (
                  <Badge variant="light" color="green" size="sm">
                    {performanceInfo}
                  </Badge>
                )}
              </Group>
            </Box>
            {onRun && (
              <Button
                onClick={handleRun}
                disabled={isRunning}
                loading={isRunning}
                size="sm"
              >
                {isRunning ? 'Running...' : 'Run'}
              </Button>
            )}
          </Group>
          <Text size="sm" c="dimmed" mt="xs">{description}</Text>
        </Card.Section>
        <Card.Section inheritPadding py="md">
          <Stack gap="md">
            {/* Code Section */}
            <Box>
              <Text size="sm" fw={600} mb="xs">Code:</Text>
              <CodeHighlight
                code={code}
                language="javascript"
                withCopyButton
              />
            </Box>

            {/* Result Section */}
            {(result || error) && (
              <Box>
                <Text size="sm" fw={600} mb="xs">
                  {error ? 'Error:' : 'Result:'}
                </Text>
                <Box
                  p="sm"
                  style={{
                    backgroundColor: error
                      ? 'rgba(239, 68, 68, 0.1)'
                      : 'rgba(34, 197, 94, 0.1)',
                    borderRadius: 'var(--mantine-radius-sm)',
                    border: `1px solid ${error ? 'var(--mantine-color-red-6)' : 'var(--mantine-color-green-6)'}`
                  }}
                >
                  <Text
                    size="sm"
                    ff="monospace"
                    c={error ? 'red' : 'green'}
                    style={{ whiteSpace: 'pre-wrap' }}
                  >
                    {error || result}
                  </Text>
                </Box>
              </Box>
            )}
          </Stack>
        </Card.Section>
      </Card>
    </ErrorBoundary>
  );
}

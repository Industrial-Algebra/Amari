import { Title, Text, Card, Button, SimpleGrid, Stack, List, Group, Container } from "@mantine/core";
import { Link } from "react-router-dom";

export function Home() {
  return (
    <Container size="lg" py="xl">
      <Stack gap="xl">
        <div>
          <Title order={1} mb="sm">Amari Mathematical Computing Library</Title>
          <Text size="lg" c="dimmed">
            Interactive API examples and documentation for exotic number systems and algebraic structures
          </Text>
        </div>

        <SimpleGrid cols={{ base: 1, md: 2 }} spacing="lg">
          <Card withBorder>
            <Card.Section withBorder inheritPadding py="sm">
              <Title order={3}>Core Mathematics</Title>
            </Card.Section>
            <Card.Section inheritPadding py="md">
              <Text mb="md">Explore the fundamental algebraic structures that power the Amari library:</Text>
              <List size="sm" spacing="xs" mb="md">
                <List.Item>Geometric Algebra (Clifford Algebra)</List.Item>
                <List.Item>Tropical Algebra (Max-Plus Semiring)</List.Item>
                <List.Item>Dual Number Automatic Differentiation</List.Item>
                <List.Item>CGT &amp; Short Surreals</List.Item>
                <List.Item>Rational Surcomplex (v0.23)</List.Item>
                <List.Item>Information Geometry</List.Item>
              </List>
              <Button component={Link} to="/geometric-algebra">
                Start with Geometric Algebra
              </Button>
            </Card.Section>
          </Card>

          <Card withBorder>
            <Card.Section withBorder inheritPadding py="sm">
              <Title order={3}>Advanced Systems</Title>
            </Card.Section>
            <Card.Section inheritPadding py="md">
              <Text mb="md">Discover high-performance computing and integration features:</Text>
              <List size="sm" spacing="xs" mb="md">
                <List.Item>WebGPU Acceleration</List.Item>
                <List.Item>TropicalDualClifford Fusion</List.Item>
                <List.Item>Cellular Automata</List.Item>
                <List.Item>Edge Computing</List.Item>
              </List>
              <Button component={Link} to="/webgpu">
                Explore WebGPU
              </Button>
            </Card.Section>
          </Card>
        </SimpleGrid>

        <Card withBorder>
          <Card.Section withBorder inheritPadding py="sm">
            <Title order={3}>Quick Start</Title>
          </Card.Section>
          <Card.Section inheritPadding py="md">
            <Text mb="md">
              The Amari library provides a unified framework for mathematical computing with exotic number systems.
              Each module is designed to work independently or as part of the integrated fusion system.
            </Text>
            <Group gap="md">
              <Button component={Link} to="/playground">
                Interactive Playground
              </Button>
              <Button component={Link} to="/api-reference" variant="outline">
                API Reference
              </Button>
            </Group>
          </Card.Section>
        </Card>
      </Stack>
    </Container>
  );
}

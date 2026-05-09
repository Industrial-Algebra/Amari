import { Link, useLocation } from "react-router-dom";
import { Title, Text, Stack, Box, UnstyledButton } from "@mantine/core";

interface NavSection {
  title: string;
  items: NavItem[];
}

interface NavItem {
  title: string;
  href: string;
  description: string;
}

const navigationSections: NavSection[] = [
  {
    title: "Core Mathematics",
    items: [
      {
        title: "Geometric Algebra",
        href: "/geometric-algebra",
        description: "Multivectors, geometric products, and rotors"
      },
      {
        title: "Tropical Algebra",
        href: "/tropical-algebra",
        description: "Max-plus semiring operations"
      },
      {
        title: "Dual Numbers",
        href: "/dual-numbers",
        description: "Automatic differentiation"
      },
      {
        title: "CGT & Short Surreals",
        href: "/cgt-surreal",
        description: "Short games, nimbers, and exact dyadics"
      },
      {
        title: "Information Geometry",
        href: "/information-geometry",
        description: "Statistical manifolds and Fisher metrics"
      },
      {
        title: "Enumerative Geometry",
        href: "/enumerative-geometry",
        description: "Intersection theory and curve counting"
      },
      {
        title: "Calculus",
        href: "/calculus",
        description: "Differential operators and integration"
      },
      {
        title: "Measure Theory",
        href: "/measure",
        description: "Measures, probability, and information"
      },
      {
        title: "Topology",
        href: "/topology",
        description: "Homology, persistence, and Morse theory"
      },
      {
        title: "Dynamics",
        href: "/dynamics",
        description: "Chaos, attractors, and bifurcations"
      }
    ]
  },
  {
    title: "Advanced Systems",
    items: [
      {
        title: "WebGPU Acceleration",
        href: "/webgpu",
        description: "GPU-accelerated computations"
      },
      {
        title: "Fusion System",
        href: "/fusion",
        description: "TropicalDualClifford integration"
      },
      {
        title: "Cellular Automata",
        href: "/automata",
        description: "Self-assembling systems"
      },
      {
        title: "Probabilistic",
        href: "/probabilistic",
        description: "Distributions and stochastic processes"
      },
      {
        title: "Relativistic",
        href: "/relativistic",
        description: "Special and general relativity"
      },
      {
        title: "Network",
        href: "/network",
        description: "Geometric network analysis"
      },
      {
        title: "Holographic",
        href: "/holographic",
        description: "Distributed memory and resonators"
      },
      {
        title: "Optimization",
        href: "/optimization",
        description: "Gradient and geodesic optimization"
      }
    ]
  },
  {
    title: "Tools & Playground",
    items: [
      {
        title: "Visualizations",
        href: "/visualizations",
        description: "Interactive math demonstrations"
      },
      {
        title: "Interactive Playground",
        href: "/playground",
        description: "Live code examples and experimentation"
      },
      {
        title: "Performance Benchmarks",
        href: "/benchmarks",
        description: "Compare algorithm performance"
      },
      {
        title: "API Reference",
        href: "/api-reference",
        description: "Complete function documentation"
      },
      {
        title: "Amari-Chentsov Tensor",
        href: "/amari-chentsov-tensor",
        description: "Interactive tensor visualization"
      }
    ]
  }
];

interface NavigationProps {
  onNavigate?: () => void;
}

export function Navigation({ onNavigate }: NavigationProps) {
  const location = useLocation();

  return (
    <Box p="md">
      <Box mb="xl">
        <UnstyledButton component={Link} to="/" onClick={onNavigate}>
          <Title order={3} c="cyan">Amari Library</Title>
          <Text size="sm" c="dimmed">Mathematical Computing Examples</Text>
        </UnstyledButton>
      </Box>

      <Stack gap="lg">
        {navigationSections.map((section) => (
          <Box key={section.title}>
            <Text
              size="xs"
              fw={600}
              tt="uppercase"
              c="dimmed"
              mb="sm"
              style={{ letterSpacing: '0.1em' }}
            >
              {section.title}
            </Text>
            <Stack gap="xs">
              {section.items.map((item) => {
                const isActive = location.pathname === item.href;
                return (
                  <UnstyledButton
                    key={item.href}
                    component={Link}
                    to={item.href}
                    onClick={onNavigate}
                    p="sm"
                    style={(theme) => ({
                      borderRadius: theme.radius.sm,
                      backgroundColor: isActive
                        ? theme.colors.dark[6]
                        : 'transparent',
                      border: isActive
                        ? `1px solid ${theme.colors.cyan[7]}`
                        : '1px solid transparent',
                      transition: 'all 0.15s ease',
                      '&:hover': {
                        backgroundColor: theme.colors.dark[6],
                      }
                    })}
                  >
                    <Text size="sm" fw={500} mb={2}>
                      {item.title}
                    </Text>
                    <Text size="xs" c="dimmed">
                      {item.description}
                    </Text>
                  </UnstyledButton>
                );
              })}
            </Stack>
          </Box>
        ))}
      </Stack>
    </Box>
  );
}

import React from 'react'
import ReactDOM from 'react-dom/client'
import { createBrowserRouter, RouterProvider } from 'react-router-dom'
import { MantineProvider, createTheme } from '@mantine/core'
import { CodeHighlightAdapterProvider, createShikiAdapter } from '@mantine/code-highlight'
import { createHighlighter } from 'shiki'
import '@mantine/core/styles.css'
import '@mantine/code-highlight/styles.css'

import App from './App'
import { GeometricAlgebra } from './pages/GeometricAlgebra'
import { TropicalAlgebra } from './pages/TropicalAlgebra'
import { CgtSurreal } from './pages/CgtSurreal'
import { Surcomplex } from './pages/Surcomplex'
import { DualNumbers } from './pages/DualNumbers'
import { InformationGeometry } from './pages/InformationGeometry'
import { WebGPU } from './pages/WebGPU'
import { Fusion } from './pages/Fusion'
import { Automata } from './pages/Automata'
import { Playground } from './pages/Playground'
import { Benchmarks } from './pages/Benchmarks'
import { APIReference } from './pages/APIReference'
import { Home } from './pages/Home'
import { EnumerativeGeometry } from './pages/EnumerativeGeometry'
import { AmariChentsovTensorDemo } from './pages/AmariChentsovTensorDemo'
import { Probabilistic } from './pages/Probabilistic'
import { Calculus } from './pages/Calculus'
import { Relativistic } from './pages/Relativistic'
import { Network } from './pages/Network'
import { Measure } from './pages/Measure'
import { Holographic } from './pages/Holographic'
import { Optimization } from './pages/Optimization'
import { Topology } from './pages/Topology'
import { Dynamics } from './pages/Dynamics'
import { Visualizations } from './pages/Visualizations'

// Scientific dark theme for mathematical computing
const theme = createTheme({
  primaryColor: 'cyan',
  fontFamily: 'Inter, system-ui, -apple-system, sans-serif',
  fontFamilyMonospace: 'JetBrains Mono, Menlo, Monaco, monospace',
  headings: {
    fontFamily: 'Inter, system-ui, -apple-system, sans-serif',
    fontWeight: '600',
  },
  colors: {
    dark: [
      '#C1C2C5',
      '#A6A7AB',
      '#909296',
      '#5c5f66',
      '#373A40',
      '#2C2E33',
      '#25262b',
      '#1A1B1E',
      '#141517',
      '#101113',
    ],
  },
  other: {
    codeBackground: '#1e1e2e',
  },
})

const router = createBrowserRouter([
  {
    path: "/",
    element: <App />,
    children: [
      {
        index: true,
        element: <Home />,
      },
      {
        path: "geometric-algebra",
        element: <GeometricAlgebra />,
      },
      {
        path: "tropical-algebra",
        element: <TropicalAlgebra />,
      },
      {
        path: "cgt-surreal",
        element: <CgtSurreal />,
      },
      {
        path: "surcomplex",
        element: <Surcomplex />,
      },
      {
        path: "dual-numbers",
        element: <DualNumbers />,
      },
      {
        path: "information-geometry",
        element: <InformationGeometry />,
      },
      {
        path: "webgpu",
        element: <WebGPU />,
      },
      {
        path: "fusion",
        element: <Fusion />,
      },
      {
        path: "automata",
        element: <Automata />,
      },
      {
        path: "playground",
        element: <Playground />,
      },
      {
        path: "benchmarks",
        element: <Benchmarks />,
      },
      {
        path: "api-reference",
        element: <APIReference />,
      },
      {
        path: "enumerative-geometry",
        element: <EnumerativeGeometry />,
      },
      {
        path: "amari-chentsov-tensor",
        element: <AmariChentsovTensorDemo />,
      },
      {
        path: "probabilistic",
        element: <Probabilistic />,
      },
      {
        path: "calculus",
        element: <Calculus />,
      },
      {
        path: "relativistic",
        element: <Relativistic />,
      },
      {
        path: "network",
        element: <Network />,
      },
      {
        path: "measure",
        element: <Measure />,
      },
      {
        path: "holographic",
        element: <Holographic />,
      },
      {
        path: "optimization",
        element: <Optimization />,
      },
      {
        path: "topology",
        element: <Topology />,
      },
      {
        path: "dynamics",
        element: <Dynamics />,
      },
      {
        path: "visualizations",
        element: <Visualizations />,
      },
      {
        path: "*",
        element: <Home />,
      },
    ],
  },
])

const shikiAdapter = createShikiAdapter(async () => {
  const highlighter = await createHighlighter({
    langs: ['javascript', 'typescript', 'rust'],
    themes: [],
  })
  return highlighter
})

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <MantineProvider theme={theme} defaultColorScheme="dark">
      <CodeHighlightAdapterProvider adapter={shikiAdapter}>
        <RouterProvider router={router} />
      </CodeHighlightAdapterProvider>
    </MantineProvider>
  </React.StrictMode>,
)

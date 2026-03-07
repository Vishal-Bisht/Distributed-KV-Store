import { useState } from "react";
import { ThemeProvider, createTheme } from "@mui/material/styles";
import CssBaseline from "@mui/material/CssBaseline";
import ClusterOverview from "./components/ClusterOverview";
import KeyValueBrowser from "./components/KeyValueBrowser";
import HubIcon from "@mui/icons-material/Hub";
import GitHubIcon from "@mui/icons-material/GitHub";
import StorageIcon from "@mui/icons-material/Storage";

// Dark theme configuration
const darkTheme = createTheme({
  palette: {
    mode: "dark",
    primary: {
      main: "#3b82f6",
    },
    secondary: {
      main: "#8b5cf6",
    },
    background: {
      default: "#0a0e17",
      paper: "#1a1f2e",
    },
  },
  typography: {
    fontFamily:
      '"Inter", "SF Pro Display", -apple-system, BlinkMacSystemFont, sans-serif',
  },
});

function App() {
  const [selectedNode, setSelectedNode] = useState(null);
  const [activeTab, setActiveTab] = useState("cluster");
  const [refreshTrigger, setRefreshTrigger] = useState(0);

  const handleRefreshCluster = () => {
    setRefreshTrigger((prev) => prev + 1);
  };

  return (
    <ThemeProvider theme={darkTheme}>
      <CssBaseline />
      <div
        className="flex flex-col min-h-screen w-full bg-gradient-to-br from-slate-950 via-slate-900 to-slate-950"
        style={{ padding: "0" }}
      >
        {/* Header */}
        <header
          className="w-full border-b border-gray-800/50 backdrop-blur-xl bg-gray-900/30 sticky top-0 z-40"
          style={{ paddingLeft: "2rem", paddingRight: "2rem" }}
        >
          <div className="w-full py-4">
            <div className="flex items-center justify-between">
              {/* Logo */}
              <div className="flex items-center gap-4">
                <div className="relative">
                  <div className="absolute inset-0 bg-gradient-to-br from-blue-500 to-purple-600 rounded-xl blur-lg opacity-50" />
                  <div className="relative p-3 bg-gradient-to-br from-blue-500 to-purple-600 rounded-xl">
                    <HubIcon className="w-6 h-6 text-white" />
                  </div>
                </div>
                <div>
                  <h1 className="text-xl font-bold bg-gradient-to-r from-white to-gray-400 bg-clip-text text-transparent">
                    Distributed KV Store
                  </h1>
                  <p className="text-xs text-gray-500">
                    Raft Consensus Dashboard
                  </p>
                </div>
              </div>

              {/* Navigation */}
              <nav className="flex items-center gap-2 bg-gray-800/50 p-1 rounded-xl">
                <button
                  onClick={() => setActiveTab("cluster")}
                  className={`flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium transition-all ${
                    activeTab === "cluster"
                      ? "bg-blue-500 text-white shadow-lg shadow-blue-500/25"
                      : "text-gray-400 hover:text-white hover:bg-gray-700/50"
                  }`}
                >
                  <HubIcon className="w-4 h-4" />
                  Cluster
                </button>
                <button
                  onClick={() => setActiveTab("data")}
                  className={`flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium transition-all ${
                    activeTab === "data"
                      ? "bg-emerald-500 text-white shadow-lg shadow-emerald-500/25"
                      : "text-gray-400 hover:text-white hover:bg-gray-700/50"
                  }`}
                >
                  <StorageIcon className="w-4 h-4" />
                  Data
                </button>
              </nav>

              {/* GitHub Link */}
              <a
                href="https://github.com/Vishal-Bisht/Distributed-KV-Store"
                target="_blank"
                rel="noopener noreferrer"
                className="p-2 rounded-lg bg-gray-800/50 hover:bg-gray-700/50 transition-colors"
              >
                <GitHubIcon className="w-5 h-5 text-gray-400 hover:text-white" />
              </a>
            </div>
          </div>
        </header>

        {/* Main Content */}
        <main
          className="flex-1 w-full py-8"
          style={{ paddingLeft: "2rem", paddingRight: "2rem" }}
        >
          {/* Tab Content */}
          <div className="flex flex-col gap-6">
            {activeTab === "cluster" ? (
              <>
                <div>
                  <ClusterOverview
                    onNodeSelect={setSelectedNode}
                    selectedNode={selectedNode}
                    refreshTrigger={refreshTrigger}
                  />
                </div>
              </>
            ) : (
              <>
                <div>
                  <KeyValueBrowser
                    selectedNode={selectedNode}
                    onRefreshCluster={handleRefreshCluster}
                  />
                </div>
              </>
            )}
          </div>
        </main>

        {/* Footer */}
        <footer
          className="w-full mt-auto border-t border-gray-800/50"
          style={{ paddingLeft: "2rem", paddingRight: "2rem" }}
        >
          <div className="w-full py-6">
            <div className="flex flex-col md:flex-row items-center justify-between">
              <p className="text-sm text-gray-500">
                Built with Rust + Raft Consensus • React Dashboard
              </p>
              <div className="flex items-center">
                <div className="flex items-center gap-2">
                  <div className="w-2 h-2 rounded-full bg-emerald-400 animate-pulse" />
                  <span className="text-xs text-gray-500">Live Updates</span>
                </div>
                <span className="text-xs text-gray-600">v1.0.0</span>
              </div>
            </div>
          </div>
        </footer>
      </div>
    </ThemeProvider>
  );
}

export default App;

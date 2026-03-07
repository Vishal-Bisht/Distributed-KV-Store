import { useState, useEffect } from "react";
import NodeCard from "./NodeCard";
import RefreshIcon from "@mui/icons-material/Refresh";
import HubIcon from "@mui/icons-material/Hub";
import apiService from "../services/api";

const ClusterOverview = ({ onNodeSelect, selectedNode, refreshTrigger }) => {
  const [nodes, setNodes] = useState([]);
  const [loading, setLoading] = useState(true);
  const [lastUpdate, setLastUpdate] = useState(null);

  const fetchStatus = async () => {
    setLoading(true);
    try {
      const statuses = await apiService.getAllNodesStatus();
      setNodes(statuses);
      setLastUpdate(new Date());
    } catch (error) {
      console.error("Failed to fetch cluster status:", error);
    }
    setLoading(false);
  };

  useEffect(() => {
    fetchStatus();
    const interval = setInterval(fetchStatus, 2000); // Poll every 2 seconds
    return () => clearInterval(interval);
  }, [refreshTrigger]);

  const onlineCount = nodes.filter((n) => n.online).length;
  const leader = nodes.find((n) => n.data?.is_leader);
  const totalTerm = Math.max(
    ...nodes.filter((n) => n.online).map((n) => n.data?.node?.term || 0),
    0,
  );

  return (
    <div className="flex flex-col w-full gap-8">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="p-3 bg-gradient-to-br from-purple-500/20 to-blue-500/20 rounded-xl">
            <HubIcon className="w-6 h-6 text-purple-400" />
          </div>
          <div>
            <h2 className="text-xl font-bold text-white">Cluster Status</h2>
            <p className="text-sm text-gray-400">
              {lastUpdate
                ? `Updated ${lastUpdate.toLocaleTimeString()}`
                : "Loading..."}
            </p>
          </div>
        </div>

        <button
          onClick={fetchStatus}
          disabled={loading}
          className="p-2 rounded-lg bg-gray-800/50 hover:bg-gray-700/50 transition-colors disabled:opacity-50"
        >
          <RefreshIcon
            className={`w-5 h-5 text-gray-400 ${loading ? "animate-spin" : ""}`}
          />
        </button>
      </div>

      {/* Quick Stats */}
      <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
        <div className="bg-gray-800/30 rounded-xl p-4 border border-gray-700/50">
          <p className="text-xs text-gray-400 mb-1">Online Nodes</p>
          <p className="text-2xl font-bold">
            <span
              className={onlineCount >= 2 ? "text-emerald-400" : "text-red-400"}
            >
              {onlineCount}
            </span>
            <span className="text-gray-500">/{nodes.length}</span>
          </p>
        </div>

        <div className="bg-gray-800/30 rounded-xl p-4 border border-gray-700/50">
          <p className="text-xs text-gray-400 mb-1">Current Leader</p>
          <p className="text-2xl font-bold text-emerald-400">
            {leader ? `Node ${leader.id}` : "None"}
          </p>
        </div>

        <div className="bg-gray-800/30 rounded-xl p-4 border border-gray-700/50">
          <p className="text-xs text-gray-400 mb-1">Current Term</p>
          <p className="text-2xl font-bold text-blue-400 font-mono">
            {totalTerm}
          </p>
        </div>
      </div>

      {/* Cluster Health Indicator */}
      <div
        className={`
        p-3 rounded-lg flex items-center gap-3
        ${onlineCount >= 2 ? "bg-emerald-500/10 border border-emerald-500/30" : "bg-red-500/10 border border-red-500/30"}
      `}
      >
        <div
          className={`
          w-2 h-2 rounded-full
          ${onlineCount >= 2 ? "bg-emerald-400 animate-pulse" : "bg-red-400"}
        `}
        />
        <p
          className={`text-sm font-medium ${onlineCount >= 2 ? "text-emerald-400" : "text-red-400"}`}
        >
          {onlineCount >= 2
            ? "Cluster is healthy - Quorum achieved"
            : "Cluster degraded - No quorum"}
        </p>
      </div>

      {/* Node Cards */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
        {nodes.map((node) => (
          <NodeCard
            key={node.id}
            node={node}
            isSelected={selectedNode?.id === node.id}
            onClick={() => onNodeSelect(node)}
          />
        ))}
      </div>

      {/* Network Topology Visualization */}
      <div className="p-6 bg-gray-800/30 rounded-xl border border-gray-700/50">
        <h3 className="text-sm font-semibold text-gray-400 mb-4">
          Network Topology
        </h3>
        <div className="flex justify-center items-center">
          <svg width="300" height="150" className="overflow-visible">
            {(() => {
              // Dynamic positions based on leader
              const leaderId = leader?.id || 1;
              const followerIds = [1, 2, 3].filter((id) => id !== leaderId);

              // Leader at top center, followers at bottom
              const positions = {
                [leaderId]: { x: 150, y: 30 },
                [followerIds[0]]: { x: 50, y: 120 },
                [followerIds[1]]: { x: 250, y: 120 },
              };

              const leaderPos = positions[leaderId];
              const follower1Pos = positions[followerIds[0]];
              const follower2Pos = positions[followerIds[1]];

              return (
                <>
                  {/* Connection lines from leader to followers */}
                  {leader && (
                    <>
                      <line
                        x1={leaderPos.x}
                        y1={leaderPos.y}
                        x2={follower1Pos.x}
                        y2={follower1Pos.y}
                        className="stroke-emerald-500 data-flow-line"
                        strokeWidth="2"
                      />
                      <line
                        x1={leaderPos.x}
                        y1={leaderPos.y}
                        x2={follower2Pos.x}
                        y2={follower2Pos.y}
                        className="stroke-emerald-500 data-flow-line"
                        strokeWidth="2"
                      />
                    </>
                  )}

                  {/* Dashed line between followers */}
                  <line
                    x1={follower1Pos.x}
                    y1={follower1Pos.y}
                    x2={follower2Pos.x}
                    y2={follower2Pos.y}
                    className="stroke-gray-600"
                    strokeWidth="2"
                    strokeDasharray="5,5"
                  />

                  {/* Node circles */}
                  {[1, 2, 3].map((id) => {
                    const pos = positions[id];
                    const node = nodes.find((n) => n.id === id);
                    const isOnline = node?.online;
                    const isLeaderNode = node?.data?.is_leader;
                    return (
                      <g key={id}>
                        <circle
                          cx={pos.x}
                          cy={pos.y}
                          r={isLeaderNode ? 25 : 20}
                          className={`
                            ${
                              !isOnline
                                ? "fill-gray-700"
                                : isLeaderNode
                                  ? "fill-emerald-500"
                                  : "fill-blue-500"
                            }
                            ${isLeaderNode ? "animate-pulse" : ""}
                          `}
                        />
                        <text
                          x={pos.x}
                          y={pos.y + 5}
                          textAnchor="middle"
                          className="fill-white text-sm font-bold"
                        >
                          {id}
                        </text>
                      </g>
                    );
                  })}
                </>
              );
            })()}
          </svg>
        </div>
        <div className="flex justify-center gap-6 mt-4">
          <div className="flex items-center gap-2">
            <div className="w-3 h-3 rounded-full bg-emerald-500" />
            <span className="text-xs text-gray-400">Leader</span>
          </div>
          <div className="flex items-center gap-2">
            <div className="w-3 h-3 rounded-full bg-blue-500" />
            <span className="text-xs text-gray-400">Follower</span>
          </div>
          <div className="flex items-center gap-2">
            <div className="w-3 h-3 rounded-full bg-gray-700" />
            <span className="text-xs text-gray-400">Offline</span>
          </div>
        </div>
      </div>
    </div>
  );
};

export default ClusterOverview;

import { useState, useEffect } from "react";
import AddIcon from "@mui/icons-material/Add";
import DeleteIcon from "@mui/icons-material/Delete";
import RefreshIcon from "@mui/icons-material/Refresh";
import SearchIcon from "@mui/icons-material/Search";
import KeyIcon from "@mui/icons-material/Key";
import CheckCircleIcon from "@mui/icons-material/CheckCircle";
import ErrorIcon from "@mui/icons-material/Error";
import DataObjectIcon from "@mui/icons-material/DataObject";
import apiService from "../services/api";

const KeyValueBrowser = ({ selectedNode, onRefreshCluster }) => {
  const [keys, setKeys] = useState([]);
  const [loading, setLoading] = useState(false);
  const [searchTerm, setSearchTerm] = useState("");
  const [showAddModal, setShowAddModal] = useState(false);
  const [newKey, setNewKey] = useState("");
  const [newValue, setNewValue] = useState("");
  const [notification, setNotification] = useState(null);
  const [actionLoading, setActionLoading] = useState(false);

  const nodeUrl = selectedNode?.httpUrl || "http://127.0.0.1:9001";

  const fetchKeys = async () => {
    setLoading(true);
    try {
      const result = await apiService.getAllKeys(nodeUrl);
      if (result.success) {
        setKeys(result.data || []);
      } else {
        showNotification("error", `Failed to fetch keys: ${result.error}`);
      }
    } catch (error) {
      showNotification("error", "Failed to connect to node");
    }
    setLoading(false);
  };

  useEffect(() => {
    fetchKeys();
  }, [selectedNode]);

  const showNotification = (type, message) => {
    setNotification({ type, message });
    setTimeout(() => setNotification(null), 4000);
  };

  const handleAdd = async () => {
    if (!newKey.trim()) {
      showNotification("error", "Key cannot be empty");
      return;
    }

    setActionLoading(true);
    try {
      const result = await apiService.smartPut(newKey, newValue);
      if (result.success) {
        showNotification("success", `Key "${newKey}" stored successfully`);
        setNewKey("");
        setNewValue("");
        setShowAddModal(false);
        setTimeout(fetchKeys, 500); // Wait for replication
        onRefreshCluster?.();
      } else {
        showNotification("error", result.error);
      }
    } catch (error) {
      showNotification("error", "Failed to store key");
    }
    setActionLoading(false);
  };

  const handleDelete = async (key) => {
    if (!confirm(`Delete key "${key}"?`)) return;

    setActionLoading(true);
    try {
      const result = await apiService.smartDelete(key);
      if (result.success) {
        showNotification("success", `Key "${key}" deleted`);
        setTimeout(fetchKeys, 500);
        onRefreshCluster?.();
      } else {
        showNotification("error", result.error);
      }
    } catch (error) {
      showNotification("error", "Failed to delete key");
    }
    setActionLoading(false);
  };

  const filteredKeys = keys.filter(
    (kv) =>
      kv.key.toLowerCase().includes(searchTerm.toLowerCase()) ||
      kv.value.toLowerCase().includes(searchTerm.toLowerCase()),
  );

  return (
    <div className="flex flex-col w-full gap-8">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="p-3 bg-gradient-to-br from-emerald-500/20 to-cyan-500/20 rounded-xl">
            <DataObjectIcon className="w-6 h-6 text-emerald-400" />
          </div>
          <div>
            <h2 className="text-xl font-bold text-white">Key-Value Store</h2>
            <p className="text-sm text-gray-400">
              {keys.length} key{keys.length !== 1 ? "s" : ""} stored
            </p>
          </div>
        </div>

        <div className="flex gap-2">
          <button
            onClick={fetchKeys}
            disabled={loading}
            className="p-2 rounded-lg bg-gray-800/50 hover:bg-gray-700/50 transition-colors disabled:opacity-50"
          >
            <RefreshIcon
              className={`w-5 h-5 text-gray-400 ${loading ? "animate-spin" : ""}`}
            />
          </button>
          <button
            onClick={() => setShowAddModal(true)}
            className="flex items-center gap-2 px-4 py-2 rounded-lg bg-emerald-500 hover:bg-emerald-600 transition-colors text-white font-medium"
          >
            <AddIcon className="w-5 h-5" />
            Add Key
          </button>
        </div>
      </div>

      {/* Search */}
      <div className="relative">
        <SearchIcon className="absolute left-4 top-1/2 -translate-y-1/2 w-5 h-5 text-gray-500" />
        <input
          type="text"
          placeholder="Search keys or values..."
          value={searchTerm}
          onChange={(e) => setSearchTerm(e.target.value)}
          className="w-full pl-12 pr-4 py-3 bg-gray-800/50 border border-gray-700/50 rounded-xl text-white placeholder-gray-500 focus:outline-none focus:border-blue-500/50 transition-colors"
        />
      </div>

      {/* Notification */}
      {notification && (
        <div
          className={`
          flex items-center gap-3 p-4 rounded-xl
          ${
            notification.type === "success"
              ? "bg-emerald-500/10 border border-emerald-500/30 text-emerald-400"
              : "bg-red-500/10 border border-red-500/30 text-red-400"
          }
        `}
        >
          {notification.type === "success" ? (
            <CheckCircleIcon className="w-5 h-5" />
          ) : (
            <ErrorIcon className="w-5 h-5" />
          )}
          <p>{notification.message}</p>
        </div>
      )}

      {/* Keys Table */}
      <div className="bg-gray-800/30 rounded-xl border border-gray-700/50 overflow-hidden">
        {/* Table Header */}
        <div className="grid grid-cols-12 gap-4 p-4 bg-gray-800/50 border-b border-gray-700/50">
          <div className="col-span-4 text-xs font-semibold text-gray-400 uppercase tracking-wider">
            Key
          </div>
          <div className="col-span-6 text-xs font-semibold text-gray-400 uppercase tracking-wider">
            Value
          </div>
          <div className="col-span-2 text-xs font-semibold text-gray-400 uppercase tracking-wider text-right">
            Actions
          </div>
        </div>

        {/* Table Body */}
        <div className="divide-y divide-gray-700/50 max-h-96 overflow-y-auto">
          {loading ? (
            <div className="p-8 text-center text-gray-500">
              <RefreshIcon className="w-8 h-8 mx-auto mb-2 animate-spin" />
              <p>Loading keys...</p>
            </div>
          ) : filteredKeys.length === 0 ? (
            <div className="p-8 text-center text-gray-500">
              <KeyIcon className="w-8 h-8 mx-auto mb-2 opacity-50" />
              <p>
                {searchTerm
                  ? "No matching keys found"
                  : "No keys stored yet. Add your first key!"}
              </p>
            </div>
          ) : (
            filteredKeys.map((kv) => (
              <div
                key={kv.key}
                className="grid grid-cols-12 gap-4 p-4 hover:bg-gray-800/30 transition-colors group"
              >
                <div className="col-span-4 flex items-center gap-2">
                  <KeyIcon className="w-4 h-4 text-blue-400 flex-shrink-0" />
                  <span className="font-mono text-sm text-white truncate">
                    {kv.key}
                  </span>
                </div>
                <div className="col-span-6">
                  <span className="font-mono text-sm text-gray-300 break-all">
                    {kv.value}
                  </span>
                </div>
                <div className="col-span-2 flex justify-end">
                  <button
                    onClick={() => handleDelete(kv.key)}
                    disabled={actionLoading}
                    className="p-2 rounded-lg opacity-0 group-hover:opacity-100 bg-red-500/10 hover:bg-red-500/20 text-red-400 transition-all disabled:opacity-50"
                  >
                    <DeleteIcon className="w-4 h-4" />
                  </button>
                </div>
              </div>
            ))
          )}
        </div>
      </div>

      {/* Add Modal */}
      {showAddModal && (
        <div className="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center z-50">
          <div className="bg-gray-900 border border-gray-700 rounded-2xl p-6 w-full max-w-md mx-4 shadow-2xl">
            <h3 className="text-xl font-bold text-white mb-6">Add New Key</h3>

            <div className="space-y-4">
              <div>
                <label className="block text-sm font-medium text-gray-400 mb-2">
                  Key
                </label>
                <input
                  type="text"
                  value={newKey}
                  onChange={(e) => setNewKey(e.target.value)}
                  placeholder="Enter key name"
                  className="w-full px-4 py-3 bg-gray-800 border border-gray-700 rounded-xl text-white placeholder-gray-500 focus:outline-none focus:border-blue-500 transition-colors"
                  autoFocus
                />
              </div>

              <div>
                <label className="block text-sm font-medium text-gray-400 mb-2">
                  Value
                </label>
                <textarea
                  value={newValue}
                  onChange={(e) => setNewValue(e.target.value)}
                  placeholder="Enter value"
                  rows={3}
                  className="w-full px-4 py-3 bg-gray-800 border border-gray-700 rounded-xl text-white placeholder-gray-500 focus:outline-none focus:border-blue-500 transition-colors resize-none"
                />
              </div>
            </div>

            <div className="flex gap-3 mt-6">
              <button
                onClick={() => {
                  setShowAddModal(false);
                  setNewKey("");
                  setNewValue("");
                }}
                className="flex-1 px-4 py-3 rounded-xl bg-gray-800 hover:bg-gray-700 text-gray-300 font-medium transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={handleAdd}
                disabled={actionLoading || !newKey.trim()}
                className="flex-1 px-4 py-3 rounded-xl bg-emerald-500 hover:bg-emerald-600 text-white font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {actionLoading ? "Saving..." : "Save"}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

export default KeyValueBrowser;

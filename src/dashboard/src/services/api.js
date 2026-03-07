import axios from "axios";

// Default node addresses (HTTP API ports are TCP port + 1000)
const DEFAULT_NODES = [
  { id: 1, httpUrl: "http://127.0.0.1:9001" },
  { id: 2, httpUrl: "http://127.0.0.1:9002" },
  { id: 3, httpUrl: "http://127.0.0.1:9003" },
];

class ApiService {
  constructor() {
    this.nodes = DEFAULT_NODES;
    this.timeout = 3000;
  }

  setNodes(nodes) {
    this.nodes = nodes;
  }

  // Get status from a specific node
  async getNodeStatus(nodeUrl) {
    try {
      const response = await axios.get(`${nodeUrl}/api/status`, {
        timeout: this.timeout,
      });
      return { success: true, data: response.data };
    } catch (error) {
      return {
        success: false,
        error: error.code === "ECONNABORTED" ? "Timeout" : "Offline",
      };
    }
  }

  // Get status from all nodes
  async getAllNodesStatus() {
    const results = await Promise.all(
      this.nodes.map(async (node) => {
        const status = await this.getNodeStatus(node.httpUrl);
        return {
          ...node,
          ...status,
          online: status.success,
        };
      }),
    );
    return results;
  }

  // Get all keys from a node
  async getAllKeys(nodeUrl) {
    try {
      const response = await axios.get(`${nodeUrl}/api/keys`, {
        timeout: this.timeout,
      });
      return { success: true, data: response.data.keys };
    } catch (error) {
      return { success: false, error: error.message };
    }
  }

  // Get a specific key
  async getKey(nodeUrl, key) {
    try {
      const response = await axios.get(
        `${nodeUrl}/api/kv/${encodeURIComponent(key)}`,
        {
          timeout: this.timeout,
        },
      );
      return { success: true, data: response.data };
    } catch (error) {
      if (error.response?.status === 404) {
        return { success: false, error: "Key not found" };
      }
      return { success: false, error: error.message };
    }
  }

  // Put a key-value pair
  async putKey(nodeUrl, key, value) {
    try {
      const response = await axios.post(
        `${nodeUrl}/api/kv/${encodeURIComponent(key)}`,
        { value },
        { timeout: this.timeout },
      );
      return { success: true, data: response.data };
    } catch (error) {
      if (error.response?.status === 503) {
        return {
          success: false,
          error: error.response.data.message || "Not leader",
          notLeader: true,
        };
      }
      return { success: false, error: error.message };
    }
  }

  // Delete a key
  async deleteKey(nodeUrl, key) {
    try {
      const response = await axios.delete(
        `${nodeUrl}/api/kv/${encodeURIComponent(key)}`,
        { timeout: this.timeout },
      );
      return { success: true, data: response.data };
    } catch (error) {
      if (error.response?.status === 503) {
        return {
          success: false,
          error: error.response.data.message || "Not leader",
          notLeader: true,
        };
      }
      return { success: false, error: error.message };
    }
  }

  // Find the leader node URL
  async findLeaderUrl() {
    const statuses = await this.getAllNodesStatus();
    const leader = statuses.find((s) => s.online && s.data?.is_leader);
    return leader?.httpUrl || null;
  }

  // Smart put - automatically finds leader
  async smartPut(key, value) {
    const leaderUrl = await this.findLeaderUrl();
    if (!leaderUrl) {
      return { success: false, error: "No leader available" };
    }
    return this.putKey(leaderUrl, key, value);
  }

  // Smart delete - automatically finds leader
  async smartDelete(key) {
    const leaderUrl = await this.findLeaderUrl();
    if (!leaderUrl) {
      return { success: false, error: "No leader available" };
    }
    return this.deleteKey(leaderUrl, key);
  }
}

export const apiService = new ApiService();
export default apiService;

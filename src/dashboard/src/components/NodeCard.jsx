import StorageIcon from "@mui/icons-material/Storage";
import CheckCircleIcon from "@mui/icons-material/CheckCircle";
import CancelIcon from "@mui/icons-material/Cancel";
import StarIcon from "@mui/icons-material/Star";
import SyncIcon from "@mui/icons-material/Sync";

const NodeCard = ({ node, isSelected, onClick }) => {
  const isOnline = node.online;
  const isLeader = node.data?.is_leader;
  const role = node.data?.node?.role || "Unknown";
  const term = node.data?.node?.term || 0;
  const logLength = node.data?.node?.log_length || 0;
  const commitIndex = node.data?.node?.commit_index || 0;

  const getRoleColor = () => {
    if (!isOnline) return "bg-gray-600";
    if (isLeader) return "bg-emerald-500";
    if (role === "Candidate") return "bg-amber-500";
    return "bg-blue-500";
  };

  const getRoleBadge = () => {
    if (!isOnline) return "Offline";
    return role;
  };

  return (
    <div
      onClick={onClick}
      className={`
        relative p-6 rounded-xl cursor-pointer transition-all duration-300 w-full
        border border-gray-700/50 overflow-hidden
        ${isSelected ? "ring-2 ring-blue-500 border-blue-500/50" : ""}
        ${isOnline ? "bg-gray-800/50 hover:bg-gray-800/70" : "bg-gray-900/50 opacity-60"}
        ${isLeader && isOnline ? "leader-pulse" : ""}
        card-hover backdrop-blur-sm
      `}
    >
      {/* Leader Crown */}
      {isLeader && isOnline && (
        <div className="absolute -top-2 -right-2">
          <StarIcon className="text-yellow-400 w-6 h-6" />
        </div>
      )}

      {/* Header */}
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-3">
          <div
            className={`p-2 rounded-lg ${isOnline ? "bg-gray-700/50" : "bg-gray-800/50"}`}
          >
            <StorageIcon
              className={`w-6 h-6 ${isOnline ? "text-blue-400" : "text-gray-500"}`}
            />
          </div>
          <div>
            <h3 className="text-lg font-semibold text-white">Node {node.id}</h3>
            <p className="text-xs text-gray-400">
              {node.httpUrl.replace("http://", "")}
            </p>
          </div>
        </div>

        {/* Status Indicator */}
        {isOnline ? (
          <CheckCircleIcon className="w-5 h-5 text-emerald-400" />
        ) : (
          <CancelIcon className="w-5 h-5 text-red-400" />
        )}
      </div>

      {/* Role Badge */}
      <div className="mb-4">
        <span
          className={`
          inline-flex items-center gap-1.5 px-4 py-1.5 rounded-full text-xs font-semibold
          whitespace-nowrap overflow-hidden text-ellipsis max-w-full
          ${getRoleColor()} text-white shadow-sm
        `}
        >
          {role === "Candidate" && (
            <SyncIcon className="w-3 h-3 animate-spin flex-shrink-0" />
          )}
          <span className="truncate">{getRoleBadge()}</span>
        </span>
      </div>

      {/* Stats */}
      {isOnline && (
        <div className="grid grid-cols-2 gap-3">
          <div className="bg-gray-900/50 rounded-lg p-3">
            <p className="text-xs text-gray-400 mb-1">Term</p>
            <p className="text-lg font-mono font-bold text-white">{term}</p>
          </div>
          <div className="bg-gray-900/50 rounded-lg p-3">
            <p className="text-xs text-gray-400 mb-1">Log</p>
            <p className="text-lg font-mono font-bold text-white">
              {logLength}
            </p>
          </div>
          <div className="col-span-2 bg-gray-900/50 rounded-lg p-3">
            <div className="flex justify-between items-center">
              <p className="text-xs text-gray-400">Commit Index</p>
              <p className="text-sm font-mono font-bold text-emerald-400">
                {commitIndex}
              </p>
            </div>
          </div>
        </div>
      )}

      {/* Offline Message */}
      {!isOnline && (
        <div className="text-center py-4">
          <p className="text-gray-500 text-sm">Node unavailable</p>
          <p className="text-gray-600 text-xs mt-1">{node.error}</p>
        </div>
      )}
    </div>
  );
};

export default NodeCard;

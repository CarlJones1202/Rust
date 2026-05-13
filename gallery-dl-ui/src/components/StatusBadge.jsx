import './StatusBadge.css';

export default function StatusBadge({ status }) {
  return (
    <span className={`status-badge ${status}`}>
      <span className="status-dot" />
      {status}
    </span>
  );
}

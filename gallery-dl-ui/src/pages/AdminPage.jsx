import { useState, useEffect, useRef, useCallback } from 'react';
import { Activity, RefreshCw, CheckCircle, XCircle, Clock, AlertTriangle, Database } from 'lucide-react';
import { listHosts, checkHost, markHostDown, regatherStashdb } from '../api';
import './AdminPage.css';

function formatTime(epochSecs) {
  if (!epochSecs) return '—';
  const d = new Date(epochSecs * 1000);
  return d.toLocaleString();
}

function timeUntil(epochSecs) {
  if (!epochSecs) return '—';
  const diff = epochSecs - Math.floor(Date.now() / 1000);
  if (diff <= 0) return 'Now';
  if (diff < 60) return `${diff}s`;
  if (diff < 3600) return `${Math.floor(diff / 60)}m ${diff % 60}s`;
  return `${Math.floor(diff / 3600)}h ${Math.floor((diff % 3600) / 60)}m`;
}

export default function AdminPage() {
  const [hosts, setHosts] = useState([]);
  const [loading, setLoading] = useState(true);
  const [checkingHost, setCheckingHost] = useState(null);
  const [markingHost, setMarkingHost] = useState(null);
  const [regathering, setRegathering] = useState(false);
  const [regatherMessage, setRegatherMessage] = useState(null);
  const pollRef = useRef(null);

  const fetchData = useCallback(async () => {
    try {
      const res = await listHosts();
      setHosts(res.hosts);
    } catch (err) {
      console.error('Failed to fetch hosts:', err);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchData();
    pollRef.current = setInterval(fetchData, 10000);
    return () => clearInterval(pollRef.current);
  }, [fetchData]);

  const handleCheck = async (host) => {
    setCheckingHost(host);
    try {
      await checkHost(host);
      await fetchData();
    } catch (err) {
      console.error('Failed to check host:', err);
    } finally {
      setCheckingHost(null);
    }
  };

  const handleMarkDown = async (host) => {
    setMarkingHost(host);
    try {
      await markHostDown(host);
      await fetchData();
    } catch (err) {
      console.error('Failed to mark host down:', err);
    } finally {
      setMarkingHost(null);
    }
  };

  const handleRegather = async () => {
    setRegathering(true);
    setRegatherMessage(null);
    try {
      const res = await regatherStashdb();
      setRegatherMessage({ type: 'success', text: res.message || 'Regather started' });
    } catch (err) {
      setRegatherMessage({ type: 'error', text: err.message || 'Failed to start regather' });
    } finally {
      setRegathering(false);
    }
  };

  if (loading) {
    return (
      <div className="admin-page">
        <div className="page-header">
          <h2>Admin</h2>
          <p>Host health monitoring</p>
        </div>
        <div className="empty-state">
          <Activity size={32} />
          <p>Loading hosts...</p>
        </div>
      </div>
    );
  }

  return (
    <div className="admin-page">
      <div className="page-header">
        <h2>Admin</h2>
        <p>Host health monitoring</p>
      </div>

      <div className="admin-section">
        <div className="section-header">
          <h3>Hosts</h3>
          <button className="btn btn-ghost" onClick={fetchData} title="Refresh">
            <RefreshCw size={16} />
          </button>
        </div>

        {hosts.length === 0 ? (
          <div className="empty-state">
            <Activity size={32} />
            <h3>No hosts monitored</h3>
            <p>Hosts appear after download requests are submitted.</p>
          </div>
        ) : (
          <table className="host-table">
            <thead>
              <tr>
                <th>Host</th>
                <th>Status</th>
                <th>Last Checked</th>
                <th>Next Check</th>
                <th>Action</th>
              </tr>
            </thead>
            <tbody>
              {hosts.map((h) => (
                <tr key={h.host}>
                  <td className="host-cell">{h.host}</td>
                  <td>
                    <span className={`status-badge ${h.is_down ? 'down' : 'up'}`}>
                      {h.is_down ? (
                        <><XCircle size={14} /> Down</>
                      ) : (
                        <><CheckCircle size={14} /> Up</>
                      )}
                    </span>
                  </td>
                  <td className="time-cell">
                    <Clock size={14} />
                    {formatTime(h.last_check_at)}
                  </td>
                  <td className="time-cell">
                    {h.is_down ? (
                      <>{timeUntil(h.next_check_at)}</>
                    ) : (
                      <span className="text-muted">On demand</span>
                    )}
                  </td>
                  <td className="action-cell">
                    <button
                      className="btn btn-sm"
                      onClick={() => handleCheck(h.host)}
                      disabled={checkingHost === h.host || markingHost === h.host}
                    >
                      {checkingHost === h.host ? (
                        <><RefreshCw size={14} className="spin" /> Checking...</>
                      ) : (
                        <><RefreshCw size={14} /> Check Now</>
                      )}
                    </button>
                    {!h.is_down && (
                      <button
                        className="btn btn-sm btn-danger"
                        onClick={() => handleMarkDown(h.host)}
                        disabled={markingHost === h.host || checkingHost === h.host}
                        title="Manually mark this host as down"
                      >
                        {markingHost === h.host ? (
                          <><RefreshCw size={14} className="spin" /> Marking...</>
                        ) : (
                          <><AlertTriangle size={14} /> Mark Down</>
                        )}
                      </button>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      <div className="admin-section" style={{ marginTop: '24px' }}>
        <div className="section-header">
          <h3>StashDB Data</h3>
        </div>
        <div style={{ padding: '20px' }}>
          <p style={{ marginBottom: '12px', color: 'var(--text-secondary)', fontSize: '0.875rem' }}>
            Forcefully re-fetch all StashDB metadata and photos for every person that has been
            imported. Old data (including photos) will be purged before downloading the latest.
          </p>
          <button
            className="btn"
            onClick={handleRegather}
            disabled={regathering}
          >
            {regathering ? (
              <><RefreshCw size={16} className="spin" /> Regathering...</>
            ) : (
              <><Database size={16} /> Regather StashDB Data</>
            )}
          </button>
          {regatherMessage && (
            <p
              style={{
                marginTop: '10px',
                fontSize: '0.875rem',
                color: regatherMessage.type === 'success' ? 'var(--success)' : 'var(--error)',
              }}
            >
              {regatherMessage.text}
            </p>
          )}
        </div>
      </div>
    </div>
  );
}

import { useState, useEffect } from 'react';
import { Search, X, ExternalLink, Check, Loader2, User } from 'lucide-react';
import { searchStashDB } from '../api';
import './StashDBSearchModal.css';

export default function StashDBSearchModal({ personName, onClose, onImport }) {
  const [query, setQuery] = useState(personName || '');
  const [results, setResults] = useState([]);
  const [loading, setLoading] = useState(false);
  const [importing, setImporting] = useState(false);
  const [selectedId, setSelectedId] = useState(null);

  useEffect(() => {
    if (query.trim().length > 2) {
      const timer = setTimeout(handleSearch, 500);
      return () => clearTimeout(timer);
    }
  }, [query]);

  const handleSearch = async () => {
    setLoading(true);
    try {
      const data = await searchStashDB(query);
      setResults(data);
    } catch (err) {
      console.error(err);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal-content stash-modal" onClick={e => e.stopPropagation()}>
        <div className="modal-header">
          <div className="header-title">
             <ExternalLink size={20} className="text-accent" />
             <h3>Import from StashDB</h3>
          </div>
          <button className="close-btn" onClick={onClose}><X size={20} /></button>
        </div>

        <div className="modal-body">
          <div className="search-input-wrapper">
            <Search size={18} />
            <input 
              type="text" 
              placeholder="Search performer name..."
              value={query}
              onChange={e => setQuery(e.target.value)}
              autoFocus
            />
            {loading && <Loader2 size={18} className="animate-spin text-muted" />}
          </div>

          <div className="results-list">
            {results.length > 0 ? (
              results.map(res => (
                <div 
                  key={res.id} 
                  className={`result-item ${selectedId === res.id ? 'selected' : ''}`}
                  onClick={() => setSelectedId(res.id)}
                >
                  <div className="result-avatar">
                    {res.image_url ? (
                      <img src={res.image_url} alt="" />
                    ) : (
                      <User size={24} />
                    )}
                  </div>
                  <div className="result-info">
                    <div className="result-name">{res.name}</div>
                    {res.disambiguation && <div className="result-disambig">{res.disambiguation}</div>}
                    <div className="result-aliases">
                      {res.aliases.slice(0, 3).join(', ')}
                      {res.aliases.length > 3 && ` +${res.aliases.length - 3}`}
                    </div>
                  </div>
                  {selectedId === res.id && <Check size={20} className="text-accent" />}
                </div>
              ))
            ) : (
              !loading && query.length > 2 && <div className="empty-results">No results found on StashDB</div>
            )}
          </div>
        </div>

        <div className="modal-footer">
          <button className="btn btn-ghost" onClick={onClose} disabled={importing}>Cancel</button>
          <button 
            className="btn btn-primary" 
            disabled={!selectedId || importing}
            onClick={async () => {
              setImporting(true);
              try {
                await onImport(selectedId);
              } finally {
                setImporting(false);
              }
            }}
          >
            {importing ? (
              <>
                <Loader2 size={16} className="animate-spin" />
                Importing...
              </>
            ) : 'Import Metadata'}
          </button>
        </div>
      </div>
    </div>
  );
}

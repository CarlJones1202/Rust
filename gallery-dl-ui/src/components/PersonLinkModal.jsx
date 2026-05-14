import { useState, useEffect } from 'react';
import { Search, X, User, Check, Loader2 } from 'lucide-react';
import { listPersons, linkGalleryPerson, thumbnailUrl } from '../api';

export default function PersonLinkModal({ galleryId, onClose, onLink }) {
  const [query, setQuery] = useState('');
  const [results, setResults] = useState([]);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    handleSearch();
  }, [query]);

  const handleSearch = async () => {
    setLoading(true);
    try {
      const data = await listPersons(1, 10, query);
      setResults(data.data);
    } catch (err) {
      console.error(err);
    } finally {
      setLoading(false);
    }
  };

  const handleLink = async (personId) => {
    try {
      await linkGalleryPerson(personId, galleryId);
      onLink();
    } catch (err) {
      alert(`Failed to link: ${err.message}`);
    }
  };

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal-content" onClick={e => e.stopPropagation()}>
        <div className="modal-header">
           <div className="header-title">
             <User size={20} className="text-accent" />
             <h3>Link Person to Gallery</h3>
           </div>
           <button className="close-btn" onClick={onClose}><X size={20} /></button>
        </div>

        <div className="modal-body">
          <div className="search-input-wrapper">
            <Search size={18} />
            <input 
              type="text" 
              placeholder="Search people..."
              value={query}
              onChange={e => setQuery(e.target.value)}
              autoFocus
            />
            {loading && <Loader2 size={18} className="animate-spin text-muted" />}
          </div>

          <div className="results-list">
            {results.length > 0 ? (
              results.map(person => (
                <div 
                  key={person.id} 
                  className="result-item"
                  onClick={() => handleLink(person.id)}
                >
                  <div className="result-avatar">
                     <User size={20} />
                  </div>
                  <div className="result-info">
                    <div className="result-name">{person.name}</div>
                    {person.disambiguation && <div className="result-disambig">{person.disambiguation}</div>}
                  </div>
                  <Check size={18} className="text-muted opacity-0 group-hover:opacity-100" />
                </div>
              ))
            ) : (
              !loading && <div className="empty-results">No people found</div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

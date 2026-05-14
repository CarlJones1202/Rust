import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { Users, Search, Plus, User } from 'lucide-react';
import { listPersons, personImageUrl } from '../api';
import MediaGrid from '../components/MediaGrid';
import Pagination from '../components/Pagination';
import './PeoplePage.css';

export default function PeoplePage() {
  const [data, setData] = useState(null);
  const [page, setPage] = useState(1);
  const [search, setSearch] = useState('');
  const [debouncedSearch, setDebouncedSearch] = useState('');
  const [loading, setLoading] = useState(true);
  const navigate = useNavigate();

  useEffect(() => {
    const timer = setTimeout(() => setDebouncedSearch(search), 300);
    return () => clearTimeout(timer);
  }, [search]);

  useEffect(() => {
    setLoading(true);
    listPersons(page, 24, debouncedSearch)
      .then(setData)
      .catch(console.error)
      .finally(() => setLoading(false));
  }, [page, debouncedSearch]);

  const handleCreatePerson = async () => {
    const name = prompt('Enter person name:');
    if (!name) return;
    
    try {
      // We'll use a direct create call if we had one exported, but let's just use the api.js one
      const { createPerson } = await import('../api');
      const person = await createPerson(name);
      navigate(`/people/${person.id}`);
    } catch (err) {
      alert(`Failed to create person: ${err.message}`);
    }
  };

  if (loading && !data) {
    return <div className="empty-state"><p>Loading...</p></div>;
  }

  return (
    <div className="people-page">
      <div className="page-header">
        <div className="header-main">
          <h2>People</h2>
          <p>Manage performers and link them to galleries</p>
        </div>
        <button className="btn btn-primary btn-with-icon" onClick={handleCreatePerson}>
          <Plus size={18} />
          Add Person
        </button>
      </div>

      <div className="search-bar">
        <Search size={18} />
        <input
          type="text"
          placeholder="Search people by name or alias..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
      </div>

      {data?.data.length === 0 ? (
        <div className="empty-state">
          <Users size={48} />
          <h3>No people found</h3>
          <p>{search ? 'Try a different search term' : 'Start by adding a new person or importing from StashDB'}</p>
        </div>
      ) : (
        <>
          <MediaGrid
            items={data?.data || []}
            onItemClick={(person) => navigate(`/people/${person.id}`)}
            renderItem={(person) => {
              return (
                <div className="person-card-inner">
                   {person.image_hash ? (
                      <img 
                        src={personImageUrl(person.image_hash, person.image_extension)} 
                        alt={person.name}
                        className="person-grid-img"
                      />
                   ) : (
                      <div className="person-avatar-large">
                        <User size={48} />
                      </div>
                   )}
                  <div className="overlay">
                    <div className="overlay-text">
                      <div className="person-name">{person.name}</div>
                      {person.disambiguation && (
                        <div className="person-disambiguation">{person.disambiguation}</div>
                      )}
                    </div>
                  </div>
                </div>
              );
            }}
          />
          {data?.pagination && (
            <Pagination
              page={data.pagination.page}
              totalPages={data.pagination.total_pages}
              total={data.pagination.total}
              onPageChange={setPage}
            />
          )}
        </>
      )}
    </div>
  );
}

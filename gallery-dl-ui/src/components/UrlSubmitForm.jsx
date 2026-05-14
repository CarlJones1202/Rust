import { useState, useEffect } from 'react';
import { Send, List } from 'lucide-react';
import { createRequest, guessRequestTitle } from '../api';
import './UrlSubmitForm.css';

export default function UrlSubmitForm() {
  const [url, setUrl] = useState('');
  const [name, setName] = useState('');
  const [bulkUrls, setBulkUrls] = useState('');
  const [bulkMode, setBulkMode] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [feedback, setFeedback] = useState(null);

  useEffect(() => {
    if (bulkMode || !url.trim() || !url.includes('vipergirls.to')) {
      return;
    }

    const timer = setTimeout(async () => {
      try {
        const { title } = await guessRequestTitle(url.trim());
        if (title && !name) {
          setName(title);
        }
      } catch (err) {
        console.error('Failed to guess title:', err);
      }
    }, 800);

    return () => clearTimeout(timer);
  }, [url, bulkMode]);

  const showFeedback = (type, message) => {
    setFeedback({ type, message });
    setTimeout(() => setFeedback(null), 4000);
  };

  const submitSingle = async (targetUrl, targetName = null) => {
    const trimmed = targetUrl.trim();
    if (!trimmed) return;
    await createRequest(trimmed, targetName);
    return trimmed;
  };

  const handleSubmit = async (e) => {
    e.preventDefault();
    setSubmitting(true);
    try {
      if (bulkMode) {
        const urls = bulkUrls
          .split('\n')
          .map((u) => u.trim())
          .filter((u) => u.length > 0);
        if (urls.length === 0) {
          showFeedback('error', 'No URLs provided');
          setSubmitting(false);
          return;
        }
        let successCount = 0;
        let duplicateCount = 0;
        for (const u of urls) {
          try {
            await submitSingle(u);
            successCount++;
          } catch (err) {
            if (err.message === 'URL already exists') {
              duplicateCount++;
            }
          }
        }
        setBulkUrls('');
        if (duplicateCount > 0) {
          alert(`${duplicateCount} URL(s) were already submitted and skipped.`);
        }
        showFeedback('success', `Queued ${successCount} new URLs`);
      } else {
        try {
          await submitSingle(url, name);
          setUrl('');
          setName('');
          showFeedback('success', 'URL queued for download');
        } catch (err) {
          if (err.message === 'URL already exists') {
            alert(`This URL has already been submitted.`);
            setUrl('');
          } else {
            throw err;
          }
        }
      }
    } catch (err) {
      showFeedback('error', err.message || 'Failed to submit');
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <form className="url-form" onSubmit={handleSubmit}>
      <div className="url-form-inputs">
        {bulkMode ? (
          <textarea
             value={bulkUrls}
            onChange={(e) => setBulkUrls(e.target.value)}
            placeholder="Paste URLs here, one per line..."
            disabled={submitting}
          />
        ) : (
          <div className="url-input-row">
            <input
              type="text"
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              placeholder="Paste a URL to download..."
              disabled={submitting}
              className="url-input-main"
            />
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="Gallery name (optional)..."
              disabled={submitting}
              className="url-input-name"
            />
          </div>
        )}
        {feedback && (
          <div className={`submit-feedback ${feedback.type}`}>
            {feedback.message}
          </div>
        )}
      </div>
      <button
        type="button"
        className={`btn btn-ghost ${bulkMode ? 'active' : ''}`}
        onClick={() => setBulkMode(!bulkMode)}
        title="Toggle bulk mode"
      >
        <List size={18} />
      </button>
      <button type="submit" className="btn btn-primary" disabled={submitting}>
        <Send size={16} />
        {submitting ? 'Sending...' : 'Submit'}
      </button>
    </form>
  );
}

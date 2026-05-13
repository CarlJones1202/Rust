import { useState, useEffect } from 'react';
import { Image } from 'lucide-react';
import Lightbox from 'yet-another-react-lightbox';
import 'yet-another-react-lightbox/styles.css';
import { listImages, imageUrl } from '../api';
import MediaGrid from '../components/MediaGrid';
import Pagination from '../components/Pagination';
import './ImagesPage.css';

export default function ImagesPage() {
  const [data, setData] = useState(null);
  const [page, setPage] = useState(1);
  const [loading, setLoading] = useState(true);
  const [lightboxIndex, setLightboxIndex] = useState(-1);

  useEffect(() => {
    setLoading(true);
    listImages(page, 48)
      .then(setData)
      .catch(console.error)
      .finally(() => setLoading(false));
  }, [page]);

  if (loading && !data) {
    return <div className="empty-state"><p>Loading...</p></div>;
  }

  const images = data?.data || [];
  const slides = images.map((img) => ({
    src: imageUrl(img.hash, img.extension),
    alt: img.original_filename || `${img.hash}.${img.extension}`,
  }));

  return (
    <div>
      <div className="page-header">
        <h2>Images</h2>
        <p>All downloaded images across all galleries</p>
      </div>

      {images.length === 0 ? (
        <div className="empty-state">
          <Image size={48} />
          <h3>No images yet</h3>
          <p>Images will appear here after downloading URLs with image content</p>
        </div>
      ) : (
        <>
          <MediaGrid
            items={images}
            onItemClick={(_, index) => setLightboxIndex(index)}
            renderItem={(img) => (
              <>
                <img
                  src={imageUrl(img.hash, img.extension)}
                  alt={img.original_filename || ''}
                  loading="lazy"
                />
                <div className="overlay">
                  <div className="overlay-text">
                    {img.original_filename || `${img.hash}.${img.extension}`}
                  </div>
                </div>
              </>
            )}
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

      <Lightbox
        open={lightboxIndex >= 0}
        index={lightboxIndex}
        close={() => setLightboxIndex(-1)}
        slides={slides}
      />
    </div>
  );
}

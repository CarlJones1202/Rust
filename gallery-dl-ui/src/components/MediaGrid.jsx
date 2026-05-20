import { Heart } from 'lucide-react';
import './MediaGrid.css';

export default function MediaGrid({ items, onItemClick, renderItem, onFavorite, layout = 'justified' }) {
  return (
    <div className={`media-grid layout-${layout}`}>
      {items.map((item, index) => {
        // Calculate aspect ratio for justified layout
        // Default to 16:9 for videos if dimensions are missing
        const isVideo = item.extension === 'mp4' || item.extension === 'mkv' || item.extension === 'webm';
        const aspectRatio = item.width && item.height 
          ? item.width / item.height 
          : (isVideo ? 16 / 9 : 1);
          
        const style = layout === 'justified' ? {
          flexGrow: aspectRatio,
          flexBasis: `${aspectRatio * 200}px`, // 200px is the target height
        } : {};

        return (
          <div
            key={item.id || index}
            className="media-grid-item"
            style={style}
            onClick={() => onItemClick?.(item, index)}
          >
            {renderItem(item, index)}
            {onFavorite && (
              <button
                className="favorite-btn"
                onClick={(e) => {
                  e.stopPropagation();
                  onFavorite(item);
                }}
                title={item.is_favorite ? 'Remove from favorites' : 'Add to favorites'}
              >
                <Heart
                  size={18}
                  className={item.is_favorite ? 'favorited' : ''}
                />
              </button>
            )}
          </div>
        );
      })}
      {/* Spacer to fix last row stretching */}
      {layout === 'justified' && <div className="media-grid-spacer" style={{ flexGrow: 999999 }} />}
    </div>
  );
}

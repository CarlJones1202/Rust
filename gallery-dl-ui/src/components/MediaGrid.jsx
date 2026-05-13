import './MediaGrid.css';

export default function MediaGrid({ items, onItemClick, renderItem, layout = 'justified' }) {
  return (
    <div className={`media-grid layout-${layout}`}>
      {items.map((item, index) => {
        // Calculate aspect ratio for justified layout
        const aspectRatio = item.width && item.height ? item.width / item.height : 1;
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
          </div>
        );
      })}
      {/* Spacer to fix last row stretching */}
      {layout === 'justified' && <div className="media-grid-spacer" style={{ flexGrow: 999999 }} />}
    </div>
  );
}

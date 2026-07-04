// BETA-31：使用场景示例（Onboarding Step 3 共用、点击 callback 跳 SearchView）。
import React from 'react';

export interface ExampleQuery {
  query: string;
  description: string;
}

const EXAMPLES: ExampleQuery[] = [
  { query: '年假和休假规定', description: '中文找英文文档（语义跨语言）' },
  { query: 'leave policy', description: '英文同款 demo' },
  { query: '公司发票模板', description: '中文关键词找 Excel / Word' },
  { query: 'meeting notes Q3', description: '英文项目笔记' },
  { query: '演讲 ppt 决策', description: '混合 query / 跨范畴' },
];

export interface ExampleQueriesProps {
  onPick: (query: string) => void;
  onSkip?: () => void;
}

export const ExampleQueries: React.FC<ExampleQueriesProps> = ({ onPick, onSkip }) => {
  return (
    <div style={{ padding: '20px', backgroundColor: '#f0f2f5', borderRadius: '12px', color: '#1d1d1f' }}>
      <h2 style={{ fontSize: '18px', marginBottom: '8px' }}>试试这些搜索示例</h2>
      <p style={{ color: '#555', marginBottom: '16px', lineHeight: 1.6 }}>
        LociFind 能按意思找到、即使你记不清文件名也行。点一下任一示例、跳到搜索页演示。
      </p>
      <ul style={{ listStyle: 'none', padding: 0, marginBottom: '16px' }}>
        {EXAMPLES.map((ex) => (
          <li key={ex.query} style={{ marginBottom: '8px' }}>
            <button
              onClick={() => onPick(ex.query)}
              style={{
                display: 'block',
                width: '100%',
                textAlign: 'left',
                backgroundColor: 'white',
                border: '1px solid #ddd',
                borderRadius: '8px',
                padding: '12px 16px',
                cursor: 'pointer',
              }}
            >
              <div style={{ fontSize: '14px', fontWeight: 500, color: '#1d1d1f' }}>{ex.query}</div>
              <div style={{ fontSize: '12px', color: '#777', marginTop: '4px' }}>{ex.description}</div>
            </button>
          </li>
        ))}
      </ul>
      {onSkip && (
        <button
          onClick={onSkip}
          style={{
            backgroundColor: 'transparent',
            color: '#666',
            border: '1px solid #ccc',
            padding: '8px 20px',
            borderRadius: '6px',
            cursor: 'pointer',
            fontSize: '13px',
          }}
        >
          跳过、直接进入应用
        </button>
      )}
    </div>
  );
};

export default ExampleQueries;

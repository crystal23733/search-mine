import React, { useEffect } from "react"

const App:React.FC = () => {
  useEffect(() => {
    console.log('App 컴포넌트가 마운트되었습니다.');
  }, [])

  return (
    <div className="min-h-screen bg-gray-100 flex items-center justify-center">
      <h1 className="text-3xl font-bold text-gray-800">지뢰찾기</h1>
    </div>
  );
}

export default App
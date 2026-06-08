import { useState } from 'react'

function App() {
  const [count, setCount] = useState(0)

  return (
    <div className="min-h-screen bg-background text-foreground">
      <div className="container mx-auto p-8">
        <h1 className="text-4xl font-bold mb-8">MatrixCode GUI</h1>
        
        <div className="card p-6 rounded-lg border shadow-sm">
          <p className="text-muted-foreground mb-4">
            Welcome to MatrixCode Desktop Application
          </p>
          
          <div className="flex items-center gap-4">
            <button 
              onClick={() => setCount(count + 1)}
              className="px-4 py-2 bg-primary text-primary-foreground rounded-md hover:bg-primary/90"
            >
              Count is {count}
            </button>
          </div>
        </div>
      </div>
    </div>
  )
}

export default App
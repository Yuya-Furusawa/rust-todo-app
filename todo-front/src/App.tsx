import { FC, useState, useEffect } from "react"
import 'modern-css-reset'
import { ThemeProvider, createTheme } from "@mui/material/styles"
import { Box, Stack, Typography } from '@mui/material'
import { NewTodoPayload, Todo, NewLabelPayload, Label } from "./types/todos"
import TodoForm from './components/TodoForm'
import TodoList from "./components/TodoList"
import SideNav from "./components/SideNav"
import { addTodoItem, getTodoItems, updateTodoItem, deleteTodoItem } from "./lib/api/todo"
import { addLabelItem, getLabelItem, deleteLabelItem } from "./lib/api/label"

const TodoApp: FC = () => {
  const [todos, setTodos] = useState<Todo[]>([])
  const [labels, setLabels] = useState<Label[]>([])
  const [filterLabelId, setFilterLabelId] = useState<number | null>(null)

  const onSubmit = async (payload: NewTodoPayload) => {
    if (!payload.text) return

    await addTodoItem(payload)
    const todos = await getTodoItems() // APIから再度Todo配列を取得する
    setTodos(todos)
  }

  const onUpdate = async (updateTodo: Todo) => {
    await updateTodoItem(updateTodo)
    const todos = await getTodoItems() // APIから再度Todo配列を取得する
    setTodos(todos)
  }

  const onDelete = async (id: number) => {
    await deleteTodoItem(id)
    const todos = await getTodoItems() // APIから再度Todo配列を取得する
    setTodos(todos)
  }

  const onSelectLabel = (label: Label | null) => {
    setFilterLabelId(label?.id ?? null)
  }

  const onSubmitNewLabel = async (newLabel: NewLabelPayload) => {
    if (!labels.some((label) => label.name === newLabel.name)) {
      const res = await addLabelItem(newLabel)
      setLabels([...labels, res])
    }
  }

  const onDeleteLabel = async (id: number) => {
    await deleteLabelItem(id)
    setLabels((prev) => prev.filter((label) => label.id !== id))
  }

  const dispTodo = filterLabelId
    ? todos.filter((todo) =>
      todo.labels.some((label) => label.id === filterLabelId)
    )
    : todos

  useEffect(() => {
    ;(async () => {
      const todos = await getTodoItems()
      setTodos(todos)
      const labelResponse = await getLabelItem()
      setLabels(labelResponse)
    })()
  }, [])

  return (
    <>
      <Box
        sx={{
          backgroundColor: 'white',
          borderBottom: '1px solid gray',
          display: 'flex',
          alignItems: 'center',
          position: 'fixed',
          top: 0,
          p: 2,
          width: '100%',
          height: 80,
          zIndex: 3,
        }}
      >
        <Typography variant="h1">Todo App</Typography>
      </Box>
      <Box
        sx={{
          backgroundColor: 'white',
          borderRight: '1px solid gray',
          position: 'fixed',
          height: 'calc(100% - 80px',
          width: 200,
          zIndex: 2,
          left: 0,
        }}
      >
        <SideNav
          labels={labels}
          onSelectLabel={onSelectLabel}
          filterLabelId={filterLabelId}
          onSubmitNewLabel={onSubmitNewLabel}
          onDeleteLabel={onDeleteLabel}
        />
      </Box>
      <Box
        sx={{
          display: 'flex',
          justifyContent: 'center',
          p: 5,
          mt: 10,
        }}
      >
        <Box maxWidth={700} width="100%">
          <Stack spacing={5}>
            <TodoForm onSubmit={onSubmit}/>
            <TodoList
              todos={dispTodo}
              onUpdate={onUpdate}
              onDelete={onDelete}
            />
          </Stack>
        </Box>
      </Box>
    </>
  )
}

const theme = createTheme({
  typography: {
    h1: {
      fontSize: 30,
    },
    h2: {
      fontSize: 20,
    }
  }
})

const App: FC = () => {
  return (
    <ThemeProvider theme={theme}>
      <TodoApp />
    </ThemeProvider>
  )
}

export default App
